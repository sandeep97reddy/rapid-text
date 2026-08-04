// Rapid Text windows speaker input and stream
use super::AudioDevice;
use anyhow::Result;
use futures_util::Stream;
use std::collections::VecDeque;
use std::sync::{mpsc, Arc, Mutex};
use std::task::{Poll, Waker};
use std::thread;
use std::time::Duration;
use tracing::{error, warn};
use wasapi::{get_default_device, DeviceCollection, Direction, SampleType, StreamMode, WaveFormat};

pub fn get_input_devices() -> Result<Vec<AudioDevice>> {
    let mut devices = Vec::new();

    let default_device = get_default_device(&Direction::Capture).ok();
    let default_id = default_device.as_ref().and_then(|d| d.get_id().ok());

    let collection = DeviceCollection::new(&Direction::Capture)?;
    let count = collection.get_nbr_devices()?;

    for i in 0..count {
        if let Ok(device) = collection.get_device_at_index(i) {
            let name = device
                .get_friendlyname()
                .unwrap_or_else(|_| format!("Microphone {}", i));
            let id = device
                .get_id()
                .unwrap_or_else(|_| format!("windows_input_{}", i));
            let is_default = default_id.as_ref().map(|def| def == &id).unwrap_or(false);

            devices.push(AudioDevice {
                id,
                name,
                is_default,
            });
        }
    }

    Ok(devices)
}

pub fn get_output_devices() -> Result<Vec<AudioDevice>> {
    let mut devices = Vec::new();

    let default_device = get_default_device(&Direction::Render).ok();
    let default_id = default_device.as_ref().and_then(|d| d.get_id().ok());

    let collection = DeviceCollection::new(&Direction::Render)?;
    let count = collection.get_nbr_devices()?;

    for i in 0..count {
        if let Ok(device) = collection.get_device_at_index(i) {
            let name = device
                .get_friendlyname()
                .unwrap_or_else(|_| format!("Speaker {}", i));
            let id = device
                .get_id()
                .unwrap_or_else(|_| format!("windows_output_{}", i));
            let is_default = default_id.as_ref().map(|def| def == &id).unwrap_or(false);

            devices.push(AudioDevice {
                id,
                name,
                is_default,
            });
        }
    }

    Ok(devices)
}

fn find_device_by_id(direction: &Direction, device_id: &str) -> Option<wasapi::Device> {
    let collection = match DeviceCollection::new(direction) {
        Ok(c) => c,
        Err(e) => {
            error!(
                "[find_device_by_id] Failed to create device collection: {}",
                e
            );
            return None;
        }
    };

    let count = match collection.get_nbr_devices() {
        Ok(c) => c,
        Err(e) => {
            error!("[find_device_by_id] Failed to get device count: {}", e);
            return None;
        }
    };

    for i in 0..count {
        if let Ok(device) = collection.get_device_at_index(i) {
            if let Ok(id) = device.get_id() {
                if id == device_id {
                    let name = device
                        .get_friendlyname()
                        .unwrap_or_else(|_| "Unknown".to_string());
                    return Some(device);
                }
            }
        }
    }

    error!(
        "[find_device_by_id] No matching device found for ID: {}",
        device_id
    );
    None
}

pub struct SpeakerInput {
    device_id: Option<String>,
    is_mic: bool,
}

impl SpeakerInput {
    pub fn new(device_id: Option<String>, is_mic: bool) -> Result<Self> {
        // Store the device_id for later use in stream()
        let device_id = device_id.filter(|id| !id.is_empty() && id != "default");
        Ok(Self { device_id, is_mic })
    }

    // Starts the audio stream
    pub fn stream(self, active_streams_count: Arc<std::sync::atomic::AtomicUsize>) -> Result<SpeakerStream> {
        let sample_queue = Arc::new(Mutex::new(VecDeque::new()));
        let waker_state = Arc::new(Mutex::new(WakerState {
            waker: None,
            has_data: false,
            shutdown: false,
        }));
        let (init_tx, init_rx) = mpsc::channel();

        let queue_clone = sample_queue.clone();
        let waker_clone = waker_state.clone();
        let device_id = self.device_id;
        let is_mic = self.is_mic;

        let active_streams_count_clone = active_streams_count.clone();
        let capture_thread = thread::spawn(move || {
            let mut is_first_init = true;
            loop {
                // Check if shutdown was requested
                {
                    let state = waker_clone.lock().unwrap();
                    if state.shutdown {
                        break;
                    }
                }

                // Run the capture loop once
                let loop_res = SpeakerStream::capture_audio_loop_once(
                    queue_clone.clone(),
                    waker_clone.clone(),
                    &init_tx,
                    device_id.clone(),
                    is_mic,
                    is_first_init,
                );

                if is_first_init {
                    if let Err(ref e) = loop_res {
                        let _ = init_tx.send(Err(anyhow::anyhow!("{}", e)));
                        break;
                    }
                }

                is_first_init = false;

                match loop_res {
                    Ok(true) => {
                        // Exited due to route change, restart loop immediately
                        thread::sleep(Duration::from_millis(100));
                    }
                    Ok(false) => {
                        // Normal exit (shutdown), stop thread
                        break;
                    }
                    Err(e) => {
                        error!(
                            "Windows Audio capture loop encountered error: {}. Retrying in 1s...",
                            e
                        );
                        thread::sleep(Duration::from_secs(1));
                    }
                }
            }
            active_streams_count_clone.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        });

        let actual_sample_rate = match init_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(rate)) => rate,
            Ok(Err(e)) => {
                error!("Rapid Text Audio initialization failed: {}", e);
                return Err(e);
            }
            Err(_) => {
                error!("Rapid Text Audio initialization timeout");
                return Err(anyhow::anyhow!("Audio initialization timed out"));
            }
        };

        active_streams_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        Ok(SpeakerStream {
            sample_queue,
            waker_state,
            capture_thread: Some(capture_thread),
            actual_sample_rate,
            active_streams_count,
        })
    }
}

struct WakerState {
    waker: Option<Waker>,
    has_data: bool,
    shutdown: bool,
}

pub struct SpeakerStream {
    sample_queue: Arc<Mutex<VecDeque<f32>>>,
    waker_state: Arc<Mutex<WakerState>>,
    capture_thread: Option<thread::JoinHandle<()>>,
    actual_sample_rate: u32,
    active_streams_count: Arc<std::sync::atomic::AtomicUsize>,
}

impl SpeakerStream {
    pub fn sample_rate(&self) -> u32 {
        self.actual_sample_rate
    }

    fn capture_audio_loop_once(
        sample_queue: Arc<Mutex<VecDeque<f32>>>,
        waker_state: Arc<Mutex<WakerState>>,
        init_tx: &mpsc::Sender<Result<u32>>,
        device_id: Option<String>,
        is_mic: bool,
        send_init_signal: bool,
    ) -> Result<bool> {
        let (h_event, render_client, sample_rate, initial_device_id, channels, direction, use_timer_mode) = {
            let direction = if is_mic {
                Direction::Capture
            } else {
                Direction::Render
            };
            let device = match device_id {
                Some(ref id) => match find_device_by_id(&direction, id) {
                    Some(d) => d,
                    None => get_default_device(&direction)?,
                },
                None => get_default_device(&direction)?,
            };

            let initial_device_id = device.get_id()?;

            let mut audio_client = device.get_iaudioclient()?;
            let device_format = audio_client.get_mixformat()?;
            let actual_rate = device_format.get_samplespersec();
            let channels = device_format.wave_fmt.Format.nChannels;
            let desired_format = WaveFormat::new(
                32,
                32,
                &SampleType::Float,
                actual_rate as usize,
                channels as usize,
                None,
            );

            let mut use_timer_mode = !is_mic;

            let init_res = if use_timer_mode {
                let fallback_mode = StreamMode::PollingShared {
                    autoconvert: true,
                    buffer_duration_hns: 0,
                };
                audio_client.initialize_client(&desired_format, &Direction::Capture, &fallback_mode)
            } else {
                let mode = StreamMode::EventsShared {
                    autoconvert: true,
                    buffer_duration_hns: 0,
                };
                match audio_client.initialize_client(&desired_format, &Direction::Capture, &mode) {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        warn!("EventsShared initialization failed ({:?}). Falling back to PollingShared...", e);
                        use_timer_mode = true;
                        let fallback_mode = StreamMode::PollingShared {
                            autoconvert: true,
                            buffer_duration_hns: 0,
                        };
                        audio_client.initialize_client(&desired_format, &Direction::Capture, &fallback_mode)
                    }
                }
            };
            init_res?;

            let h_event = if !use_timer_mode {
                Some(audio_client.set_get_eventhandle()?)
            } else {
                None
            };
            let render_client = audio_client.get_audiocaptureclient()?;
            audio_client.start_stream()?;

            (
                h_event,
                render_client,
                actual_rate,
                initial_device_id,
                channels,
                direction,
                use_timer_mode,
            )
        };

        if send_init_signal {
            let _ = init_tx.send(Ok(sample_rate));
        }

        let mut last_check = std::time::Instant::now();
        let mut route_changed = false;

        let mut temp_queue = VecDeque::with_capacity(8192);
        let mut samples = Vec::with_capacity(2048);
        let mut mono_samples = Vec::with_capacity(2048);
        let ch_count = channels as usize;

        loop {
            {
                let state = waker_state.lock().unwrap();
                if state.shutdown {
                    break;
                }
            }

            // Route change tracking: if capturing default, check if default device has changed
            if device_id.is_none() && last_check.elapsed() > Duration::from_secs(2) {
                last_check = std::time::Instant::now();
                if let Ok(current_default) = get_default_device(&direction) {
                    if let Ok(current_id) = current_default.get_id() {
                        if current_id != initial_device_id {
                            error!("Audio route changed (new default device: {}). Rebuilding stream...", current_id);
                            route_changed = true;
                            break;
                        }
                    }
                }
            }

            if !use_timer_mode {
                if let Some(ref event) = h_event {
                    if event.wait_for_event(3000).is_err() {
                        // Event wait failed (likely a timeout because no audio is playing).
                        // Do not stop the loop; just continue waiting.
                        continue;
                    }
                }
            } else {
                // Polling/timer-driven mode: Sleep for standard 15ms interval
                thread::sleep(Duration::from_millis(15));
            }

            temp_queue.clear();
            if let Err(e) = render_client.read_from_device_to_deque(&mut temp_queue) {
                error!("Rapid Text Failed to read audio data: {}", e);
                break;
            }

            if temp_queue.is_empty() {
                continue;
            }

            samples.clear();
            while temp_queue.len() >= 4 {
                let bytes = [
                    temp_queue.pop_front().unwrap(),
                    temp_queue.pop_front().unwrap(),
                    temp_queue.pop_front().unwrap(),
                    temp_queue.pop_front().unwrap(),
                ];
                let sample = f32::from_le_bytes(bytes);
                samples.push(sample);
            }

            if !samples.is_empty() {
                // Fast downmix multichannel to mono using chunking
                mono_samples.clear();
                if ch_count <= 1 {
                    mono_samples.extend_from_slice(&samples);
                } else {
                    for chunk in samples.chunks(ch_count) {
                        let sum: f32 = chunk.iter().sum();
                        mono_samples.push(sum / (ch_count as f32));
                    }
                }

                if !mono_samples.is_empty() {
                    let dropped = {
                        let mut queue = sample_queue.lock().unwrap();
                        let max_buffer_size = 131072;

                        queue.extend(mono_samples.iter());

                        let dropped_count = if queue.len() > max_buffer_size {
                            let to_drop = queue.len() - max_buffer_size;
                            queue.drain(0..to_drop);
                            to_drop
                        } else {
                            0
                        };

                        dropped_count
                    };

                    if dropped > 0 {
                        error!("Windows buffer overflow - dropped {} samples", dropped);
                    }

                    // Wake up consumer
                    {
                        let mut state = waker_state.lock().unwrap();
                        if !state.has_data {
                            state.has_data = true;
                            if let Some(waker) = state.waker.take() {
                                drop(state);
                                waker.wake();
                            }
                        }
                    }
                }
            }
        }

        Ok(route_changed)
    }
}

// Drops the audio stream
impl Drop for SpeakerStream {
    fn drop(&mut self) {
        let mut state = self.waker_state.lock().unwrap();
        state.shutdown = true;
    }
}

// Stream of f32 audio samples from the speaker
impl Stream for SpeakerStream {
    type Item = f32;

    // Polls the audio stream
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        {
            let state = self.waker_state.lock().unwrap();
            if state.shutdown {
                return Poll::Ready(None);
            }
        }

        {
            let mut queue = self.sample_queue.lock().unwrap();
            if let Some(sample) = queue.pop_front() {
                return Poll::Ready(Some(sample));
            }
        }

        {
            let mut state = self.waker_state.lock().unwrap();
            if state.shutdown {
                return Poll::Ready(None);
            }
            state.has_data = false;
            state.waker = Some(cx.waker().clone());
            drop(state);
        }

        {
            let mut queue = self.sample_queue.lock().unwrap();
            match queue.pop_front() {
                Some(sample) => Poll::Ready(Some(sample)),
                None => Poll::Pending,
            }
        }
    }
}
