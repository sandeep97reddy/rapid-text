// Rapid Text AI Speech Detection, and capture system audio (speaker output) as a stream of f32 samples.
use crate::speaker::{AudioDevice, SpeakerInput};
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use futures_util::StreamExt;
use hound::{WavSpec, WavWriter};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Listener, Manager};
use tauri_plugin_shell::ShellExt;
use crate::speaker::AudioDspPipeline;
use tracing::{error, warn};

// VAD Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadConfig {
    pub enabled: bool,
    pub hop_size: usize,
    pub sensitivity_rms: f32,
    pub peak_threshold: f32,
    pub silence_chunks: usize,
    pub min_speech_chunks: usize,
    pub pre_speech_chunks: usize,
    pub noise_gate_threshold: f32,
    pub max_recording_duration_secs: u64,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hop_size: 1024,
            sensitivity_rms: 0.008, // Moderate sensitivity for laptop mics
            peak_threshold: 0.025,  // Moderate peak threshold
            silence_chunks: 45,     // ~1.0s of silence before stopping
            min_speech_chunks: 7,   // ~0.16s - captures short answers
            pre_speech_chunks: 12,  // ~0.27s - enough to catch word start
            noise_gate_threshold: 0.002, // Moderate noise filtering
            max_recording_duration_secs: 180, // 3 minutes default
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum CaptureMode {
    Meeting,
    Dictation,
    Memo,
}

#[tauri::command]
pub async fn start_system_audio_capture(
    app: AppHandle,
    vad_config: Option<VadConfig>,
    device_id: Option<String>,
    mic_device_id: Option<String>,
    capture_mode: Option<CaptureMode>,
) -> Result<(), String> {
    let state = app.state::<crate::AudioState>();

    crate::write_debug_log(format!(
        "start_system_audio_capture: capture_mode={:?}, device_id={:?}, mic_device_id={:?}",
        capture_mode, device_id, mic_device_id
    ));

    // Centralized serialization: Wait if another capture is currently starting
    let start_spin = std::time::Instant::now();
    while state.is_starting.swap(true, Ordering::SeqCst) {
        if start_spin.elapsed() > std::time::Duration::from_secs(6) {
            let msg = "Timeout waiting for other capture to start".to_string();
            crate::write_debug_log(format!("ERROR: {}", msg));
            warn!("{}", msg);
            return Err(msg);
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    struct StartingGuard {
        flag: Arc<AtomicBool>,
    }
    impl Drop for StartingGuard {
        fn drop(&mut self) {
            self.flag.store(false, Ordering::SeqCst);
        }
    }
    let _guard = StartingGuard {
        flag: state.is_starting.clone(),
    };

    // Check if already capturing (if so, stop it first to prevent lock conflicts)
    let has_existing = {
        let guard = state
            .stream_task
            .lock()
            .map_err(|e| format!("Failed to acquire lock: {}", e))?;
        guard.is_some()
    };

    if has_existing {
        crate::write_debug_log("start_system_audio_capture: Capture already running. Stopping it first...".to_string());
        stop_system_audio_capture_impl(app.clone()).await?;
    }

    // Update VAD config if provided
    if let Some(config) = vad_config {
        let mut vad_cfg = state
            .vad_config
            .lock()
            .map_err(|e| format!("Failed to acquire VAD config lock: {}", e))?;
        *vad_cfg = config;
    }

    let mode = capture_mode.unwrap_or(CaptureMode::Meeting);

    // Initialize speaker output (loopback) capture — fail-soft
    // Dictation and Memo modes DO NOT need loopback audio.
    let loopback_stream = if mode == CaptureMode::Meeting {
        match SpeakerInput::new_with_device(device_id, false) {
            Ok(input) => {
                match input.stream(state.active_streams_count.clone()) {
                    Ok(stream) => {
                        let sr = stream.sample_rate();
                        if (8000..=96000).contains(&sr) {
                            Some((stream, sr))
                        } else {
                            warn!("Loopback stream has invalid sample rate: {}. Skipping system audio capture.", sr);
                            let _ = app.emit("system-audio-unavailable", "Invalid sample rate from loopback device");
                            None
                        }
                    }
                    Err(e) => {
                        warn!("Failed to start loopback stream: {}. System audio capture disabled.", e);
                        let _ = app.emit("system-audio-unavailable", format!("System audio capture unavailable: {}", e));
                        None
                    }
                }
            }
            Err(e) => {
                warn!("Failed to create loopback input: {}. System audio capture disabled.", e);
                let _ = app.emit("system-audio-unavailable", format!("System audio device unavailable: {}", e));
                None
            }
        }
    } else {
        None // Dictation and Memo don't use loopback
    };

    // Initialize microphone input capture — also fail-soft
    let mic_stream = if cfg!(target_os = "windows") {
        match SpeakerInput::new_with_device(mic_device_id.clone(), true) {
            Ok(input_mic) => {
                match input_mic.stream(state.active_streams_count.clone()) {
                    Ok(stream_mic) => {
                        let sr_mic = stream_mic.sample_rate();
                        if (8000..=96000).contains(&sr_mic) {
                            Some((stream_mic, sr_mic))
                        } else {
                            warn!("Invalid mic sample rate: {}. Skipping mic capture.", sr_mic);
                            None
                        }
                    }
                    Err(e) => {
                        crate::write_debug_log(format!("ERROR: Failed to start microphone stream: {}", e));
                        warn!("Failed to start microphone stream: {}. Mic capture disabled.", e);
                        None
                    }
                }
            }
            Err(e) => {
                crate::write_debug_log(format!("ERROR: Failed to initialize microphone input: {}", e));
                warn!("Failed to initialize microphone: {}. Mic capture disabled.", e);
                None
            }
        }
    } else {
        None
    };

    // If BOTH streams failed to initialize, abort with an error
    if loopback_stream.is_none() && mic_stream.is_none() {
        let msg = "Failed to initialize any audio stream. Please check your audio device settings.".to_string();
        crate::write_debug_log(format!("ERROR: Both loopback and microphone streams failed to initialize. {}", msg));
        error!("{}", msg);
        return Err(msg);
    }

    // Determine the primary sample rate from whichever stream is available
    let primary_sr = loopback_stream.as_ref().map(|(_, sr)| *sr)
        .or_else(|| mic_stream.as_ref().map(|(_, sr)| *sr))
        .unwrap_or(16000);

    let app_clone = app.clone();
    let vad_config = state
        .vad_config
        .lock()
        .map_err(|e| format!("Failed to read VAD config: {}", e))?
        .clone();

    let context_str = match mode {
        CaptureMode::Meeting => "meeting",
        CaptureMode::Dictation => "dictation",
        CaptureMode::Memo => "memo",
    };

    // Mark as capturing BEFORE spawning task
    *state
        .is_capturing
        .lock()
        .map_err(|e| format!("Failed to set capturing state: {}", e))? = true;

    *state
        .active_capture_mode
        .lock()
        .map_err(|e| format!("Failed to set capture mode: {}", e))? = Some(context_str.to_string());

    crate::write_debug_log(format!("SUCCESS: Spawning stream task. Primary sample rate: {}", primary_sr));

    // Emit capture started event with the specific mode
    let _ = app_clone.emit("capture-started", context_str);

    let state_clone = app.state::<crate::AudioState>();
    let task = tokio::spawn(async move {
        struct CleanupGuard {
            app: AppHandle,
        }
        impl Drop for CleanupGuard {
            fn drop(&mut self) {
                let state = self.app.state::<crate::AudioState>();
                if let Ok(mut capturing) = state.is_capturing.lock() {
                    *capturing = false;
                }
                if let Ok(mut mode) = state.active_capture_mode.lock() {
                    *mode = None;
                }
                if let Ok(mut task) = state.stream_task.lock() {
                    *task = None;
                }
                let _ = self.app.emit("capture-stopped", ());
            }
        }
        let _guard = CleanupGuard { app: app_clone.clone() };

        let loopback_fut = async {
            if let Some((stream, sr)) = loopback_stream {
                // Memo mode always uses continuous capture (user-controlled stop).
                // Meeting/Dictation respect vad_config.enabled.
                if mode == CaptureMode::Memo || !vad_config.enabled {
                    run_continuous_capture(
                        app_clone.clone(),
                        stream,
                        sr,
                        vad_config.clone(),
                        "interviewer",
                        mode.clone(),
                    )
                    .await;
                } else {
                    run_vad_capture(
                        app_clone.clone(),
                        stream,
                        sr,
                        vad_config.clone(),
                        "interviewer",
                        mode.clone(),
                    )
                    .await;
                }
            }
        };

        let mic_fut = async {
            if let Some((mic_stream, sr_mic)) = mic_stream {
                // Memo mode always uses continuous capture (user-controlled stop).
                // Meeting/Dictation respect vad_config.enabled.
                if mode == CaptureMode::Memo || !vad_config.enabled {
                    run_continuous_capture(
                        app_clone.clone(),
                        mic_stream,
                        sr_mic,
                        vad_config.clone(),
                        "me",
                        mode.clone(),
                    )
                    .await;
                } else {
                    run_vad_capture(
                        app_clone.clone(),
                        mic_stream,
                        sr_mic,
                        vad_config.clone(),
                        "me",
                        mode.clone(),
                    )
                    .await;
                }
            }
        };

        // Run both capture streams concurrently
        tokio::join!(loopback_fut, mic_fut);
    });

    *state_clone
        .stream_task
        .lock()
        .map_err(|e| format!("Failed to store task: {}", e))? = Some(task);

    Ok(())
}

// VAD-enabled capture - OPTIMIZED for real-time speech detection
async fn run_vad_capture(
    app: AppHandle,
    stream: impl StreamExt<Item = f32> + Unpin,
    sr: u32,
    config: VadConfig,
    speaker: &'static str,
    mode: CaptureMode,
) {
    let mut stream = stream;
    let silence_chunks_threshold = std::cmp::max((config.silence_chunks as u64 * sr as u64 / 44100) as usize, 5);
    let min_speech_chunks_threshold = std::cmp::max((config.min_speech_chunks as u64 * sr as u64 / 44100) as usize, 2);
    let pre_speech_chunks_threshold = std::cmp::max((config.pre_speech_chunks as u64 * sr as u64 / 44100) as usize, 3);

    let context = match mode {
        CaptureMode::Meeting => "meeting",
        CaptureMode::Dictation => "dictation",
        CaptureMode::Memo => "memo",
    };

    let mut dsp_pipeline = AudioDspPipeline::new(sr);
    let mut buffer: VecDeque<f32> = VecDeque::new();
    let mut pre_speech: VecDeque<f32> =
        VecDeque::with_capacity(pre_speech_chunks_threshold * config.hop_size);
    let mut speech_buffer = Vec::new();
    let mut mono = Vec::with_capacity(config.hop_size);
    let mut in_speech = false;
    let mut silence_chunks = 0;
    let mut speech_chunks = 0;
    let max_samples = sr as usize * 30; // 30s safety cap per utterance

    while let Some(sample) = stream.next().await {
        buffer.push_back(sample);

        // Process in fixed chunks for VAD analysis
        while buffer.len() >= config.hop_size {
            mono.clear();
            for _ in 0..config.hop_size {
                if let Some(v) = buffer.pop_front() {
                    mono.push(v);
                }
            }

            // Apply 150Hz HPF + Noise Gate + Soft Limiter + Sensitivity Boost in-place (zero allocations per hop)
            dsp_pipeline.process_in_place(&mut mono, config.noise_gate_threshold);

            let (rms, peak) = calculate_audio_metrics(&mono);
            let is_speech = rms > config.sensitivity_rms || peak > config.peak_threshold;

            if is_speech {
                if !in_speech {
                    // Speech START detected
                    in_speech = true;
                    speech_chunks = 0;

                    // Include pre-speech buffer for natural sound
                    speech_buffer.extend(pre_speech.drain(..));

                    let _ = app.emit("speech-start", ());
                }

                speech_chunks += 1;
                speech_buffer.extend_from_slice(&mono);
                silence_chunks = 0; // Reset silence counter on any speech

                // Safety cap: force emit if exceeds 30s
                if speech_buffer.len() > max_samples {
                    dispatch_speech_segment(app.clone(), sr, speech_buffer.clone(), speaker, context);
                    speech_buffer.clear();
                    in_speech = false;
                    speech_chunks = 0;
                }
            } else {
                // Silence detected
                if in_speech {
                    silence_chunks += 1;

                    // Continue collecting during silence (important for natural speech)
                    speech_buffer.extend_from_slice(&mono);

                    // Check if silence duration exceeds threshold
                    if silence_chunks >= silence_chunks_threshold {
                        // Verify minimum speech duration
                        if speech_chunks >= min_speech_chunks_threshold && !speech_buffer.is_empty() {
                            // Trim trailing silence (keep ~0.15s for natural ending)
                            let silence_duration_samples = silence_chunks * config.hop_size;
                            let keep_silence_samples = (sr as usize) * 15 / 100; // 0.15s
                            let trim_amount =
                                silence_duration_samples.saturating_sub(keep_silence_samples);

                            if speech_buffer.len() > trim_amount {
                                speech_buffer.truncate(speech_buffer.len() - trim_amount);
                            }

                            // Emit complete speech segment via direct STT or Base64 fallback
                            dispatch_speech_segment(app.clone(), sr, speech_buffer.clone(), speaker, context);
                        } else {
                            let _ = app.emit(
                                "speech-discarded",
                                "Audio too short (likely background noise)",
                            );
                        }

                        // Reset for next speech detection
                        speech_buffer.clear();
                        in_speech = false;
                        silence_chunks = 0;
                        speech_chunks = 0;
                    }
                } else {
                    // Not in speech yet - maintain rolling pre-speech buffer
                    pre_speech.extend(mono.iter().copied());

                    // Trim excess (maintain fixed size)
                    while pre_speech.len() > pre_speech_chunks_threshold * config.hop_size {
                        pre_speech.pop_front();
                    }

                    // Periodically shrink capacity to prevent memory bloat
                    if pre_speech.len() == pre_speech_chunks_threshold * config.hop_size {
                        pre_speech.shrink_to_fit();
                    }
                }
            }
        }
    }
}

// Continuous capture (VAD disabled)
async fn run_continuous_capture(
    app: AppHandle,
    stream: impl StreamExt<Item = f32> + Unpin,
    sr: u32,
    config: VadConfig,
    speaker: &'static str,
    mode: CaptureMode,
) {
    let mut stream = stream;
    let max_samples = (sr as u64 * config.max_recording_duration_secs) as usize;
    let context = match mode {
        CaptureMode::Meeting => "meeting",
        CaptureMode::Dictation => "dictation",
        CaptureMode::Memo => "memo",
    };

    // DSP pipeline — same as run_vad_capture so Memo gets HPF + Limiter + Noise Gate
    let mut dsp_pipeline = AudioDspPipeline::new(sr);
    let mut chunk_buf: Vec<f32> = Vec::with_capacity(config.hop_size);

    // Pre-allocate buffer to prevent reallocations
    let mut audio_buffer = Vec::with_capacity(max_samples);
    let start_time = Instant::now();
    let max_duration = Duration::from_secs(config.max_recording_duration_secs);

    // Atomic flag for manual stop
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_for_listener = stop_flag.clone();

    // Listen for manual stop event
    let stop_listener = app.listen("manual-stop-continuous", move |_| {
        stop_flag_for_listener.store(true, Ordering::Release);
    });

    // Emit recording started
    let _ = app.emit(
        "continuous-recording-start",
        config.max_recording_duration_secs,
    );

    // Accumulate audio in hop-sized chunks, applying DSP each hop
    while let Some(sample) = stream.next().await {
        if stop_flag.load(Ordering::Acquire) {
            break;
        }

        chunk_buf.push(sample);

        // Process a full hop through DSP, then drain into audio_buffer
        if chunk_buf.len() >= config.hop_size {
            dsp_pipeline.process_in_place(&mut chunk_buf, config.noise_gate_threshold);
            audio_buffer.extend_from_slice(&chunk_buf);
            chunk_buf.clear();
        }

        let elapsed = start_time.elapsed();

        // Emit progress every second
        if audio_buffer.len() % (sr as usize) == 0 && !audio_buffer.is_empty() {
            let _ = app.emit("recording-progress", elapsed.as_secs());
        }

        // Check size limit (safety)
        if audio_buffer.len() >= max_samples {
            break;
        }

        // Check time limit
        if elapsed >= max_duration {
            break;
        }
    }

    // Flush any remaining partial chunk through DSP
    if !chunk_buf.is_empty() {
        dsp_pipeline.process_in_place(&mut chunk_buf, config.noise_gate_threshold);
        audio_buffer.extend_from_slice(&chunk_buf);
    }

    // Clean up event listener (CRITICAL)
    app.unlisten(stop_listener);

    // Process and emit audio
    if !audio_buffer.is_empty() {
        dispatch_speech_segment(app.clone(), sr, audio_buffer.clone(), speaker, context);
    } else {
        warn!("No audio captured in continuous mode");
        let _ = app.emit("audio-encoding-error", "No audio recorded");
    }

    let _ = app.emit("continuous-recording-stopped", ());
}

// Apply noise gate in-place (zero-allocation)
fn apply_noise_gate_in_place(samples: &mut [f32], threshold: f32) {
    if threshold <= 0.0 {
        return;
    }
    let inv_threshold = 1.0 / threshold;
    for s in samples.iter_mut() {
        let abs = s.abs();
        if abs < threshold {
            let ratio = abs * inv_threshold;
            *s *= ratio.cbrt();
        }
    }
}

// Calculate RMS and peak (optimized)
fn calculate_audio_metrics(chunk: &[f32]) -> (f32, f32) {
    let mut sumsq = 0.0f32;
    let mut peak = 0.0f32;

    for &v in chunk {
        let a = v.abs();
        peak = peak.max(a);
        sumsq += v * v;
    }

    let rms = (sumsq / chunk.len() as f32).sqrt();
    (rms, peak)
}

fn normalize_audio_level(samples: &[f32], target_rms: f32) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }

    let sum_squares: f32 = samples.iter().map(|&s| s * s).sum();
    let current_rms = (sum_squares / samples.len() as f32).sqrt();

    if current_rms < 0.001 {
        return samples.to_vec();
    }

    let gain = (target_rms / current_rms).min(10.0);

    samples
        .iter()
        .map(|&s| {
            let amplified = s * gain;
            if amplified.abs() > 1.0 {
                amplified.signum() * (1.0 - (-amplified.abs()).exp())
            } else {
                amplified
            }
        })
        .collect()
}

fn resample_to_16k(input: &[f32], from_rate: u32) -> Vec<f32> {
    let to_rate = 16000;
    if from_rate == to_rate {
        return input.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let new_len = (input.len() as f64 / ratio).round() as usize;
    let mut output = Vec::with_capacity(new_len);
    for i in 0..new_len {
        let pos = i as f64 * ratio;
        let idx = pos.floor() as usize;
        let frac = pos - idx as f64;
        if idx + 1 < input.len() {
            output.push(input[idx] * (1.0 - frac as f32) + input[idx + 1] * frac as f32);
        } else if idx < input.len() {
            output.push(input[idx]);
        }
    }
    output
}

// Convert samples to raw WAV bytes
fn samples_to_wav_bytes(sample_rate: u32, mono_f32: &[f32]) -> Result<Vec<u8>, String> {
    if !(8000..=96000).contains(&sample_rate) {
        error!("Invalid sample rate: {}", sample_rate);
        return Err(format!(
            "Invalid sample rate: {}. Expected 8000-96000 Hz",
            sample_rate
        ));
    }

    if mono_f32.is_empty() {
        return Err("Empty audio buffer".to_string());
    }

    let resampled_mono = resample_to_16k(mono_f32, sample_rate);
    let mut cursor = Cursor::new(Vec::new());
    let spec = WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = WavWriter::new(&mut cursor, spec).map_err(|e| {
        error!("Failed to create WAV writer: {}", e);
        e.to_string()
    })?;

    for &s in &resampled_mono {
        let clamped = s.clamp(-1.0, 1.0);
        let sample_i16 = (clamped * i16::MAX as f32) as i16;
        writer.write_sample(sample_i16).map_err(|e| e.to_string())?;
    }

    writer.finalize().map_err(|e| e.to_string())?;
    Ok(cursor.into_inner())
}

// Dispatches a completed speech segment: tries direct Rust STT HTTP request first, falls back to Base64 emit on error.
fn dispatch_speech_segment(
    app: AppHandle,
    sr: u32,
    samples: Vec<f32>,
    speaker: &'static str,
    context: &'static str,
) {
    let normalized = normalize_audio_level(&samples, 0.15);
    let state = app.state::<crate::AudioState>();
    let config_state = app.state::<std::sync::Arc<std::sync::Mutex<crate::dictation_state::DictationConfigState>>>();
    let config = {
        if let Ok(guard) = config_state.lock() {
            guard.clone()
        } else {
            crate::dictation_state::DictationConfigState::default()
        }
    };

    let key_index = state.stt_key_index.clone();

    // Construct STT config dynamically
    let cfg = crate::api::ActiveSttConfig {
        url: config.stt_url.clone(),
        keys: config.stt_keys.clone(),
        model: config.stt_model.clone(),
        auth_header_name: None,
        auth_header_prefix: None,
        extra_fields: None,
        response_json_path: None,
        prompt: None,
        is_rust_compatible: true,
    };

    if !cfg.keys.is_empty() {
        tokio::spawn(async move {
            let norm_clone = normalized.clone();
            let wav_bytes = match tokio::task::spawn_blocking(move || {
                samples_to_wav_bytes(sr, &norm_clone)
            })
            .await
            {
                Ok(Ok(bytes)) => bytes,
                _ => {
                    let _ = app.emit("audio-encoding-error", "Failed to encode WAV bytes");
                    return;
                }
            };

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_default();

            match crate::api::send_direct_stt_request(&client, &cfg, &wav_bytes, &key_index).await {
                Ok(text) => {
                    // LLM Cleanup
                    let cleaned_text = match crate::api::clean_transcript_with_llm(&client, &text, &config).await {
                        Ok(cleaned) => cleaned,
                        Err(e) => {
                            crate::write_debug_log(format!("LLM cleanup failed: {}", e));
                            text.clone()
                        }
                    };

                    // Save voice note
                    let app_clone = app.clone();
                    let raw_trans = text.clone();
                    let clean_trans = cleaned_text.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = crate::api::save_voice_note(&app_clone, &raw_trans, &clean_trans).await;
                    });

                    // Paste & restore
                    let config_clone = config.clone();
                    let paste_text = cleaned_text.clone();
                    tauri::async_runtime::spawn(async move {
                        crate::api::paste_and_restore_clipboard(&paste_text, &config_clone).await;
                    });

                    // Emit success
                    let _ = app.emit(
                        "stt-complete",
                        serde_json::json!({
                            "context": context,
                            "speaker": speaker,
                            "text": cleaned_text,
                            "success": true
                        }),
                    );
                }
                Err(err) => {
                    warn!("Direct STT request failed: {}", err);
                    let _ = app.emit("audio-encoding-error", format!("STT failed: {}", err));
                }
            }
        });
    } else {
        let _ = app.emit("system-audio-unavailable", "Please configure Groq STT keys in settings dashboard");
    }
}

// Convert samples to WAV base64 (with proper error handling)
fn samples_to_wav_b64(sample_rate: u32, mono_f32: &[f32]) -> Result<String, String> {
    let bytes = samples_to_wav_bytes(sample_rate, mono_f32)?;
    Ok(B64.encode(bytes))
}

pub async fn stop_system_audio_capture_impl<R: tauri::Runtime>(
    app: AppHandle<R>,
) -> Result<(), String> {
    let state = app.state::<crate::AudioState>();

    // Abort task in separate scope to release lock immediately
    let task_opt = {
        let mut guard = state
            .stream_task
            .lock()
            .map_err(|e| format!("Failed to acquire task lock: {}", e))?;
        guard.take()
    };

    if let Some(task) = task_opt {
        task.abort();
    }

    // Wait for the threads to actually exit (max 4 seconds)
    let start = std::time::Instant::now();
    while state.active_streams_count.load(std::sync::atomic::Ordering::SeqCst) > 0 {
        if start.elapsed() > std::time::Duration::from_secs(4) {
            warn!("Timeout waiting for old audio streams to exit");
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // Additional cleanup delay (CRITICAL for mic indicator)
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    Ok(())
}

#[tauri::command]
pub async fn stop_system_audio_capture(app: AppHandle) -> Result<(), String> {
    stop_system_audio_capture_impl(app).await
}

/// Manual stop for continuous recording
#[tauri::command]
pub async fn manual_stop_continuous(app: AppHandle) -> Result<(), String> {
    let _ = app.emit("manual-stop-continuous", ());

    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

    Ok(())
}

#[tauri::command]
pub fn check_system_audio_access(_app: AppHandle) -> Result<bool, String> {
    match SpeakerInput::new() {
        Ok(_) => Ok(true),
        Err(e) => {
            error!("System audio access check failed: {}", e);
            Ok(false)
        }
    }
}

#[tauri::command]
pub async fn request_system_audio_access(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        app.shell()
            .command("open")
            .args(["x-apple.systempreferences:com.apple.preference.security?Privacy_AudioCapture"])
            .spawn()
            .map_err(|e| {
                error!("Failed to open system preferences: {}", e);
                e.to_string()
            })?;
    }
    #[cfg(target_os = "windows")]
    {
        app.shell()
            .command("ms-settings:sound")
            .spawn()
            .map_err(|e| {
                error!("Failed to open sound settings: {}", e);
                e.to_string()
            })?;
    }
    #[cfg(target_os = "linux")]
    {
        let commands = ["pavucontrol", "gnome-control-center sound"];
        let mut opened = false;

        for cmd in &commands {
            if app.shell().command(cmd).spawn().is_ok() {
                opened = true;
                break;
            }
        }

        if !opened {
            warn!("Failed to open audio settings on Linux");
        }
    }

    Ok(())
}

// VAD Configuration Management
#[tauri::command]
pub async fn get_vad_config(app: AppHandle) -> Result<VadConfig, String> {
    let state = app.state::<crate::AudioState>();
    let config = state
        .vad_config
        .lock()
        .map_err(|e| format!("Failed to get VAD config: {}", e))?
        .clone();
    Ok(config)
}

#[tauri::command]
pub async fn update_vad_config(app: AppHandle, config: VadConfig) -> Result<(), String> {
    // Validate config
    if config.sensitivity_rms < 0.0 || config.sensitivity_rms > 1.0 {
        return Err("Invalid sensitivity_rms: must be 0.0-1.0".to_string());
    }
    if config.max_recording_duration_secs > 3600 {
        return Err("Invalid max_recording_duration_secs: must be <= 3600 (1 hour)".to_string());
    }

    let state = app.state::<crate::AudioState>();
    *state
        .vad_config
        .lock()
        .map_err(|e| format!("Failed to update VAD config: {}", e))? = config;

    Ok(())
}

#[tauri::command]
pub async fn get_capture_status(app: AppHandle) -> Result<Option<String>, String> {
    let state = app.state::<crate::AudioState>();
    let mode = state
        .active_capture_mode
        .lock()
        .map_err(|e| format!("Failed to get capture status: {}", e))?;
    Ok(mode.clone())
}

#[tauri::command]
pub fn get_audio_sample_rate(app: AppHandle) -> Result<u32, String> {
    let state = app.state::<crate::AudioState>();
    let input = SpeakerInput::new().map_err(|e| {
        error!("Failed to create speaker input: {}", e);
        format!("Failed to access system audio: {}", e)
    })?;

    let stream = input.stream(state.active_streams_count.clone()).map_err(|e| {
        error!("Failed to start speaker stream: {}", e);
        format!("Failed to start speaker stream: {}", e)
    })?;
    let sr = stream.sample_rate();

    Ok(sr)
}

#[tauri::command]
pub fn get_input_devices() -> Result<Vec<AudioDevice>, String> {
    crate::speaker::list_input_devices().map_err(|e| {
        error!("Failed to get input devices: {}", e);
        format!("Failed to get input devices: {}", e)
    })
}

#[tauri::command]
pub fn get_output_devices() -> Result<Vec<AudioDevice>, String> {
    crate::speaker::list_output_devices().map_err(|e| {
        error!("Failed to get output devices: {}", e);
        format!("Failed to get output devices: {}", e)
    })
}
