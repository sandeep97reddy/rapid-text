use base64::{engine::general_purpose, Engine as _};
use futures_util::StreamExt;
use reqwest::multipart::{Form, Part};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

fn get_app_endpoint() -> Result<String, String> {
    if let Ok(endpoint) = env::var("APP_ENDPOINT") {
        return Ok(endpoint);
    }

    match option_env!("APP_ENDPOINT") {
        Some(endpoint) => Ok(endpoint.to_string()),
        None => Ok("https://github.com/Sandeep97reddy/audio_helper".to_string()), // Fallback to avoid error
    }
}

fn get_api_access_key() -> Result<String, String> {
    if let Ok(key) = env::var("API_ACCESS_KEY") {
        return Ok(key);
    }

    match option_env!("API_ACCESS_KEY") {
        Some(key) => Ok(key.to_string()),
        None => Ok("dummy-api-access-key".to_string()), // Fallback to avoid error
    }
}

// Secure storage functions
fn get_secure_storage_path(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    fs::create_dir_all(&app_data_dir)
        .map_err(|e| format!("Failed to create app data directory: {}", e))?;

    Ok(app_data_dir.join("secure_storage.json"))
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct SecureStorage {
    license_key: Option<String>,
    instance_id: Option<String>,
    selected_rapidtext_model: Option<String>,
}

pub async fn get_stored_credentials(
    app: &AppHandle,
) -> Result<(String, String, Option<Model>), String> {
    let storage_path = get_secure_storage_path(app)?;

    // BYPASS: If no license file exists, return dummy credentials so all API features work
    if !storage_path.exists() {
        return Ok((
            "rapidtext-free-license".to_string(),
            "free-instance".to_string(),
            None,
        ));
    }

    let content = fs::read_to_string(&storage_path)
        .map_err(|e| format!("Failed to read storage file: {}", e))?;

    let storage: SecureStorage = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse storage file: {}", e))?;

    // BYPASS: Use dummy fallback if stored keys are missing
    let license_key = storage
        .license_key
        .unwrap_or_else(|| "rapidtext-free-license".to_string());
    let instance_id = storage
        .instance_id
        .unwrap_or_else(|| "free-instance".to_string());

    let selected_model: Option<Model> = storage
        .selected_rapidtext_model
        .and_then(|json_str| serde_json::from_str(&json_str).ok());

    Ok((license_key, instance_id, selected_model))
}

// Audio API Structs
#[derive(Debug, Serialize, Deserialize)]
pub struct AudioResponse {
    success: bool,
    transcription: Option<String>,
    error: Option<String>,
}

// Chat API Structs
#[derive(Debug, Serialize, Deserialize)]
pub struct ChatRequest {
    user_message: String,
    system_prompt: Option<String>,
    image_base64: Option<serde_json::Value>, // Can be string or array
    history: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatResponse {
    success: bool,
    message: Option<String>,
    error: Option<String>,
}

// Model API Structs
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Model {
    provider: String,
    name: String,
    id: String,
    model: String,
    description: String,
    modality: String,
    #[serde(rename = "isAvailable")]
    is_available: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelsResponse {
    models: Vec<Model>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemPromptResponse {
    prompt_name: String,
    system_prompt: String,
}

// Rapid Text Prompts API
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RapidTextPrompt {
    title: String,
    prompt: String,
    #[serde(rename = "modelId")]
    model_id: String,
    #[serde(rename = "modelName")]
    model_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RapidTextPromptsResponse {
    prompts: Vec<RapidTextPrompt>,
    total: i32,
    #[serde(rename = "last_updated")]
    last_updated: Option<String>,
}

// API Response Configuration Structs
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponseConfig {
    url: String,
    user_token: String,
    model: String,
    body: String,
    customer_id: Option<i64>,
    customer_email: Option<String>,
    customer_name: Option<String>,
    license_key: String,
    instance_id: String,
    #[serde(rename = "user_audio")]
    user_audio: Option<UserAudioConfig>,
    errors: Option<Vec<ApiConfigError>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiConfigError {
    includes: String,
    error: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserAudioHeader {
    key: String,
    value: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserAudioConfig {
    url: String,
    #[serde(rename = "fallback_url")]
    fallback_url: Option<String>,
    model: String,
    #[serde(rename = "fallback_model")]
    fallback_model: Option<String>,
    #[serde(rename = "user_token")]
    user_token: String,
    #[serde(rename = "fallback_user_token")]
    fallback_user_token: Option<String>,
    headers: Option<Vec<UserAudioHeader>>,
}

// Active STT Configuration synced from Frontend Context
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ActiveSttConfig {
    pub url: String,
    pub keys: Vec<String>,
    pub model: String,
    pub auth_header_name: Option<String>,
    pub auth_header_prefix: Option<String>,
    pub extra_fields: Option<Vec<(String, String)>>,
    pub response_json_path: Option<String>,
    pub prompt: Option<String>,
    pub is_rust_compatible: bool,
}

use crate::dictation_state::DictationConfigState;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::sync::{Arc, Mutex};

#[tauri::command]
pub async fn sync_dictation_config_to_rust(
    app: AppHandle,
    config: DictationConfigState,
) -> Result<(), String> {
    let dictation_state = app.state::<Arc<Mutex<DictationConfigState>>>();
    {
        let mut guard = dictation_state
            .lock()
            .map_err(|e| format!("Failed to acquire DictationConfigState lock: {}", e))?;
        *guard = config.clone();
    }

    // Save to local JSON config file in App Data for fast startup reads
    if let Ok(app_data) = app.path().app_data_dir() {
        let _ = std::fs::create_dir_all(&app_data);
        let config_path = app_data.join("dictation_config.json");
        if let Ok(serialized) = serde_json::to_string_pretty(&config) {
            let _ = std::fs::write(config_path, serialized);
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn get_dictation_config(app: AppHandle) -> Result<DictationConfigState, String> {
    let dictation_state = app.state::<Arc<Mutex<DictationConfigState>>>();
    let guard = dictation_state
        .lock()
        .map_err(|e| format!("Failed to acquire DictationConfigState lock: {}", e))?;
    Ok(guard.clone())
}

pub async fn clean_transcript_with_llm(
    client: &reqwest::Client,
    raw_text: &str,
    config: &DictationConfigState,
) -> Result<String, String> {
    if config.llm_keys.is_empty() || raw_text.trim().is_empty() {
        return Ok(raw_text.to_string());
    }

    let url = if config.llm_url.is_empty() {
        "https://api.groq.com/openai/v1/chat/completions"
    } else {
        &config.llm_url
    };

    let model = if config.llm_model.is_empty() {
        "llama-3.1-8b-instant"
    } else {
        &config.llm_model
    };

    let key = &config.llm_keys[0];
    if key.trim().is_empty() {
        return Ok(raw_text.to_string());
    }

    let user_prompt = format!("<transcript>{}</transcript>", raw_text);

    let body = serde_json::json!({
        "model": model,
        "temperature": 0.0,
        "messages": [
            {
                "role": "system",
                "content": config.cleanup_prompt
            },
            {
                "role": "user",
                "content": user_prompt
            }
        ]
    });

    let resp = client
        .post(url)
        .header("Authorization", format!("Bearer {}", key.trim()))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("LLM cleanup request failed: {}", e))?;

    if !resp.status().is_success() {
        tracing::warn!("LLM cleanup HTTP error: {}, returning raw text", resp.status());
        return Ok(raw_text.to_string());
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse LLM cleanup JSON: {}", e))?;

    if let Some(clean_text) = json
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
    {
        Ok(clean_text.trim().to_string())
    } else {
        Ok(raw_text.to_string())
    }
}

#[cfg(target_os = "windows")]
pub fn get_clipboard_text() -> Option<String> {
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, OpenClipboard,
    };
    use windows_sys::Win32::System::Memory::{
        GlobalLock, GlobalUnlock,
    };
    const CF_UNICODETEXT: u32 = 13;

    unsafe {
        if OpenClipboard(std::ptr::null_mut()) != 0 {
            let h_mem = GetClipboardData(CF_UNICODETEXT);
            if !h_mem.is_null() {
                let ptr = GlobalLock(h_mem) as *const u16;
                if !ptr.is_null() {
                    let mut len = 0;
                    while *ptr.add(len) != 0 {
                        len += 1;
                    }
                    let slice = std::slice::from_raw_parts(ptr, len);
                    let text = String::from_utf16_lossy(slice);
                    GlobalUnlock(h_mem);
                    CloseClipboard();
                    return Some(text);
                }
            }
            CloseClipboard();
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
pub fn get_clipboard_text() -> Option<String> {
    None
}

#[cfg(target_os = "windows")]
pub fn set_clipboard_text(text: &str) {
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };
    const CF_UNICODETEXT: u32 = 13;

    let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let size = utf16.len() * std::mem::size_of::<u16>();

    unsafe {
        if OpenClipboard(std::ptr::null_mut()) != 0 {
            EmptyClipboard();
            let h_mem = GlobalAlloc(GMEM_MOVEABLE, size);
            if !h_mem.is_null() {
                let ptr = GlobalLock(h_mem) as *mut u16;
                if !ptr.is_null() {
                    std::ptr::copy_nonoverlapping(utf16.as_ptr(), ptr, utf16.len());
                    GlobalUnlock(h_mem);
                    SetClipboardData(CF_UNICODETEXT, h_mem as _);
                }
            }
            CloseClipboard();
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn set_clipboard_text(_text: &str) {}

pub async fn paste_and_restore_clipboard(text: &str, config: &DictationConfigState) {
    if text.trim().is_empty() {
        return;
    }

    // 1. Backup existing clipboard content
    let backup = get_clipboard_text();

    // 2. Set new text to clipboard
    set_clipboard_text(text);

    // 3. Trigger Enigo paste
    if config.auto_paste {
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
            let _ = enigo.key(Key::Control, Direction::Press);
            let _ = enigo.key(Key::Unicode('v'), Direction::Click);
            let _ = enigo.key(Key::Control, Direction::Release);
        }
    }

    // 4. Wait 150ms and restore clipboard if copy_to_clipboard is FALSE
    if !config.copy_to_clipboard {
        if let Some(backup_text) = backup {
            tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
            set_clipboard_text(&backup_text);
        }
    }
}

#[tauri::command]
pub async fn update_active_stt_config(
    app: AppHandle,
    config: ActiveSttConfig,
) -> Result<(), String> {
    let state = app.state::<crate::AudioState>();
    let mut guard = state
        .stt_config
        .lock()
        .map_err(|e| format!("Failed to acquire STT config lock: {}", e))?;
    *guard = Some(config);
    Ok(())
}

pub async fn send_direct_stt_request(
    client: &reqwest::Client,
    config: &ActiveSttConfig,
    wav_bytes: &[u8],
    key_index: &std::sync::atomic::AtomicUsize,
) -> Result<String, String> {
    if config.keys.is_empty() {
        return Err("No STT API keys configured".to_string());
    }

    let url = if config.url.is_empty() {
        "https://api.groq.com/openai/v1/audio/transcriptions"
    } else {
        &config.url
    };

    let model = if config.model.is_empty() {
        "whisper-large-v3-turbo"
    } else {
        &config.model
    };

    let mut last_error = String::new();
    let keys_len = config.keys.len();
    let start_idx = key_index.load(std::sync::atomic::Ordering::Relaxed) % keys_len;

    for i in 0..keys_len {
        let current_idx = (start_idx + i) % keys_len;
        let key = &config.keys[current_idx];
        if key.trim().is_empty() {
            continue;
        }

        let audio_part = Part::bytes(wav_bytes.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| format!("Failed to prepare audio payload: {}", e))?;

        let mut form = Form::new()
            .part("file", audio_part)
            .text("model", model.to_string())
            .text("language", "en");

        if let Some(p) = &config.prompt {
            if !p.trim().is_empty() {
                form = form.text("prompt", p.clone());
            }
        }

        if let Some(extra) = &config.extra_fields {
            for (k, v) in extra {
                form = form.text(k.clone(), v.clone());
            }
        }

        let header_name = config
            .auth_header_name
            .as_deref()
            .unwrap_or("Authorization");
        let header_prefix = config.auth_header_prefix.as_deref().unwrap_or("Bearer ");
        let auth_val = format!("{}{}", header_prefix, key.trim());

        let request = client
            .post(url)
            .header(header_name, auth_val)
            .multipart(form);

        match request.send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    let text_body = response
                        .text()
                        .await
                        .map_err(|e| format!("Failed to read STT response text: {}", e))?;

                    let json_path = config
                        .response_json_path
                        .as_deref()
                        .unwrap_or("text");

                    let extracted = if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&text_body) {
                        let pointer_path = format!("/{}", json_path.replace('.', "/"));
                        if let Some(extracted) = json_val.pointer(&pointer_path).and_then(|v| v.as_str()) {
                            extracted.to_string()
                        } else if let Some(txt) = json_val.get("text").and_then(|v| v.as_str()) {
                            txt.to_string()
                        } else if let Some(txt) = json_val.get("transcript").and_then(|v| v.as_str()) {
                            txt.to_string()
                        } else {
                            text_body.trim().to_string()
                        }
                    } else {
                        text_body.trim().to_string()
                    };

                    // Persist working key index to avoid future failover overhead
                    key_index.store(current_idx, std::sync::atomic::Ordering::Relaxed);
                    return Ok(extracted);
                } else if status.as_u16() == 429 || status.as_u16() == 402 || status.is_server_error() {
                    last_error = format!("HTTP {}: retrying next key...", status);
                    tracing::warn!("STT key rate limited/error ({}), failing over...", status);
                    continue;
                } else {
                    let err_body = response.text().await.unwrap_or_default();
                    return Err(format!("STT Error (HTTP {}): {}", status, err_body));
                }
            }
            Err(e) => {
                last_error = format!("Request error: {}", e);
                continue;
            }
        }
    }

    Err(if last_error.is_empty() {
        "All STT API keys failed".to_string()
    } else {
        last_error
    })
}

// Audio API Command
#[tauri::command]
pub async fn transcribe_audio(
    app: AppHandle,
    audio_base64: String,
    prompt: Option<String>,
) -> Result<AudioResponse, String> {
    let (_, _, selected_model) = get_stored_credentials(&app).await?;
    let provider = selected_model.as_ref().map(|model| model.provider.clone());
    let model = selected_model.as_ref().map(|model| model.model.clone());

    let api_config = fetch_api_response_config(&app, provider.clone(), model.clone()).await?;
    let user_audio_config = api_config.user_audio.as_ref().ok_or_else(|| {
        "Audio transcription is not configured for this workspace. Please contact support."
            .to_string()
    })?;

    let audio_bytes = decode_audio_base64(&audio_base64)?;
    let client = reqwest::Client::new();
    let error_provider = provider.clone();
    let error_model = model.clone();
    match perform_user_audio_transcription(
        &client,
        &user_audio_config.url,
        &user_audio_config.user_token,
        &user_audio_config.model,
        user_audio_config.headers.as_ref(),
        &audio_bytes,
        prompt.as_deref(),
    )
    .await
    {
        Ok(transcription) => Ok(AudioResponse {
            success: true,
            transcription: Some(transcription),
            error: None,
        }),
        Err(primary_error) => {
            let fallback_error_message = if let (Some(fallback_url), Some(fallback_token)) = (
                user_audio_config.fallback_url.as_ref(),
                user_audio_config.fallback_user_token.as_ref(),
            ) {
                let fallback_model = user_audio_config
                    .fallback_model
                    .as_ref()
                    .unwrap_or(&user_audio_config.model);

                match perform_user_audio_transcription(
                    &client,
                    fallback_url,
                    fallback_token,
                    fallback_model,
                    user_audio_config.headers.as_ref(),
                    &audio_bytes,
                    prompt.as_deref(),
                )
                .await
                {
                    Ok(transcription) => {
                        return Ok(AudioResponse {
                            success: true,
                            transcription: Some(transcription),
                            error: None,
                        });
                    }
                    Err(fallback_error) => Some(fallback_error),
                }
            } else {
                Some("fallback not configured".to_string())
            };

            tracing::warn!(
                primary_error = %primary_error,
                fallback_error = %fallback_error_message
                    .as_deref()
                    .unwrap_or("not attempted"),
                "Audio transcription failed for all configured endpoints"
            );
            tauri::async_runtime::spawn({
                let app = app.clone();
                let error_msg = if let Some(fallback_err) = fallback_error_message {
                    format!("Primary: {} | Fallback: {}", primary_error, fallback_err)
                } else {
                    primary_error.clone()
                };
                async move {
                    report_api_error(
                        app,
                        error_msg,
                        "/api/transcribe".to_string(),
                        error_model,
                        error_provider,
                    )
                    .await;
                }
            });
            Err("Transcription failed. Please try again.".to_string())
        }
    }
}

// Helper function to fetch API response configuration
async fn fetch_api_response_config(
    app: &AppHandle,
    provider: Option<String>,
    model: Option<String>,
) -> Result<ApiResponseConfig, String> {
    // Get environment variables
    let app_endpoint = get_app_endpoint()?;
    let api_access_key = get_api_access_key()?;
    let machine_id: String = "anonymous-open-source-machine".to_string();

    // Get stored credentials
    let (license_key, instance_id, _) = get_stored_credentials(app).await?;

    // Make HTTP request to response endpoint
    let client = reqwest::Client::new();
    let url = format!("{}/api/response", app_endpoint);

    let mut request = client
        .get(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_access_key))
        .header("license_key", &license_key)
        .header("instance", &instance_id)
        .header("machine_id", &machine_id);

    // Add optional headers
    if let Some(p) = provider {
        request = request.header("provider", p);
    }
    if let Some(m) = model {
        request = request.header("model", m);
    }

    let response = request.send().await.map_err(|e| {
        let error_msg = format!("{}", e);
        if error_msg.contains("url (") {
            let parts: Vec<&str> = error_msg.split(" for url (").collect();
            if parts.len() > 1 {
                format!("Failed to fetch API config: {}", parts[0])
            } else {
                format!("Failed to fetch API config: {}", error_msg)
            }
        } else {
            format!("Failed to fetch API config: {}", error_msg)
        }
    })?;

    // Check if the response is successful
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown server error".to_string());

        // Try to parse error as JSON to get a more specific error message
        if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&error_text) {
            if let Some(error_msg) = error_json.get("error").and_then(|e| e.as_str()) {
                return Err(format!("Server error ({}): {}", status, error_msg));
            } else if let Some(message) = error_json.get("message").and_then(|m| m.as_str()) {
                return Err(format!("Server error ({}): {}", status, message));
            }
        }

        return Err(format!("Server error ({}): {}", status, error_text));
    }
    let api_config: ApiResponseConfig = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse API config response: {}", e))?;
    Ok(api_config)
}

fn map_api_error_message(error_rules: &[ApiConfigError], sources: &[String]) -> String {
    for source in sources {
        for rule in error_rules {
            if !rule.includes.is_empty() && source.contains(&rule.includes) {
                return rule.error.clone();
            }
        }
    }

    if let Some(default_rule) = error_rules
        .iter()
        .find(|rule| rule.includes.trim().is_empty())
    {
        return default_rule.error.clone();
    }

    error_rules
        .first()
        .map(|rule| rule.error.clone())
        .unwrap_or_else(|| {
            "Something went wrong. Please try switching to a different model or contact support."
                .to_string()
        })
}

fn decode_audio_base64(audio_base64: &str) -> Result<Vec<u8>, String> {
    let trimmed = audio_base64.trim();
    let base64_str = if let Some(idx) = trimmed.find(',') {
        &trimmed[idx + 1..]
    } else {
        trimmed
    };

    general_purpose::STANDARD
        .decode(base64_str)
        .map_err(|e| format!("Failed to decode audio data: {}", e))
}

async fn perform_user_audio_transcription(
    client: &reqwest::Client,
    url: &str,
    token: &str,
    model: &str,
    headers: Option<&Vec<UserAudioHeader>>,
    audio_bytes: &[u8],
    prompt: Option<&str>,
) -> Result<String, String> {
    let audio_part = Part::bytes(audio_bytes.to_vec())
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| format!("Failed to prepare audio payload: {}", e))?;

    let mut form = Form::new()
        .part("file", audio_part)
        .text("model", model.to_string())
        .text("language", "en");

    if let Some(p) = prompt {
        if !p.trim().is_empty() {
            form = form.text("prompt", p.to_string());
        }
    }

    if let Some(extra_headers) = headers {
        for header in extra_headers {
            let key = header.key.trim();
            if key.is_empty() {
                continue;
            }

            form = form.text(key.to_string(), header.value.clone());
        }
    }

    let response = client
        .post(url)
        .bearer_auth(token)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Transcription request failed to send: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read transcription error response".to_string());
        return Err(format!(
            "Transcription request returned {} with body: {}",
            status, error_text
        ));
    }

    let body_text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read transcription response: {}", e))?;

    if body_text.trim().is_empty() {
        return Err("Transcription response was empty".to_string());
    }

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body_text) {
        if let Some(text) = json.get("text").and_then(|value| value.as_str()) {
            return Ok(text.to_string());
        }

        if let Some(text) = json
            .get("transcription")
            .and_then(|value| value.as_str())
            .or_else(|| json.get("result").and_then(|value| value.as_str()))
        {
            return Ok(text.to_string());
        }

        return Ok(json.to_string());
    }

    Ok(body_text)
}

#[tauri::command]
pub async fn chat_stream_response(
    app: AppHandle,
    user_message: String,
    system_prompt: Option<String>,
    image_base64: Option<serde_json::Value>,
    history: Option<String>,
) -> Result<String, String> {
    // Get stored credentials to get selected model
    let (_, _, selected_model) = get_stored_credentials(&app).await?;
    let (provider, model) = selected_model.as_ref().map_or((None, None), |m| {
        (Some(m.provider.clone()), Some(m.model.clone()))
    });

    // Fetch API configuration
    let api_config = fetch_api_response_config(&app, provider.clone(), model.clone()).await?;

    // Parse the body from API config to merge with our request
    let mut extra_body: serde_json::Value = if !api_config.body.is_empty() {
        serde_json::from_str(&api_config.body).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // Build messages array in OpenAI format
    let mut messages: Vec<serde_json::Value> = Vec::new();

    // Add system message if provided
    if let Some(sys_prompt) = system_prompt {
        messages.push(serde_json::json!({
            "role": "system",
            "content": sys_prompt
        }));
    }

    // Add history if provided
    if let Some(history_str) = history {
        if let Ok(history_messages) = serde_json::from_str::<Vec<serde_json::Value>>(&history_str) {
            messages.extend(history_messages);
        }
    }

    // Build user message content
    let mut user_content: Vec<serde_json::Value> = Vec::new();

    // Add text content
    user_content.push(serde_json::json!({
        "type": "text",
        "text": user_message
    }));

    // Add image content if provided
    if let Some(image_data) = image_base64 {
        if image_data.is_string() {
            // Single image
            user_content.push(serde_json::json!({
                "type": "image_url",
                "image_url": {
                    "url": format!("data:image/jpeg;base64,{}", image_data.as_str().unwrap())
                }
            }));
        } else if image_data.is_array() {
            // Multiple images
            if let Some(images) = image_data.as_array() {
                for image in images {
                    if let Some(img_str) = image.as_str() {
                        user_content.push(serde_json::json!({
                            "type": "image_url",
                            "image_url": {
                                "url": format!("data:image/jpeg;base64,{}", img_str)
                            }
                        }));
                    }
                }
            }
        }
    }

    // Add user message
    messages.push(serde_json::json!({
        "role": "user",
        "content": user_content
    }));

    // Build request body
    let mut request_body = serde_json::json!({
        "model": api_config.model,
        "messages": messages,
        "stream": true
    });

    // Merge extra body parameters from API config
    if let Some(extra_obj) = extra_body.as_object_mut() {
        if let Some(req_obj) = request_body.as_object_mut() {
            for (key, value) in extra_obj.iter() {
                req_obj.insert(key.clone(), value.clone());
            }
        }
    }

    // Make HTTP request to the configured endpoint with streaming
    let client = reqwest::Client::new();
    let error_rules = api_config.errors.clone().unwrap_or_default();
    let response = match client
        .post(&api_config.url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_config.user_token))
        .json(&request_body)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            let mut sources = vec![e.to_string()];
            if let Ok(url) = Url::parse(&api_config.url) {
                sources.push(url.to_string());
            }
            let final_message = map_api_error_message(&error_rules, &sources);
            tauri::async_runtime::spawn({
                let app = app.clone();
                let provider = provider.clone();
                let model = model.clone();
                let error_msg = e.to_string();
                async move {
                    report_api_error(app, error_msg, "/api/chat".to_string(), model, provider)
                        .await;
                }
            });
            return Err(final_message);
        }
    };

    // Check if the response is successful
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown server error".to_string());

        let mut sources = vec![error_text.clone(), status.to_string()];

        // Try to parse error as JSON to get a more specific error message
        if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&error_text) {
            if let Some(error_msg) = error_json.get("error").and_then(|e| e.as_str()) {
                sources.push(error_msg.to_string());
            }
            if let Some(message) = error_json.get("message").and_then(|m| m.as_str()) {
                sources.push(message.to_string());
            }
        }

        let final_message = map_api_error_message(&error_rules, &sources);
        tauri::async_runtime::spawn({
            let app = app.clone();
            let provider = provider.clone();
            let model = model.clone();
            let error_msg = format!("{}: {}", status, error_text);
            async move {
                report_api_error(app, error_msg, "/api/chat".to_string(), model, provider).await;
            }
        });
        return Err(final_message);
    }

    // Handle streaming response
    let mut stream = response.bytes_stream();
    let mut full_response = String::new();
    let mut buffer = String::new();
    let mut usage: Option<serde_json::Value> = None;
    let mut stream_started = false;

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => {
                let chunk_str = String::from_utf8_lossy(&bytes);
                buffer.push_str(&chunk_str);

                // Process complete lines
                let lines: Vec<&str> = buffer.split('\n').collect();
                let incomplete_line = lines.last().unwrap_or(&"").to_string();

                for line in &lines[..lines.len() - 1] {
                    // Process all but the last (potentially incomplete) line
                    let trimmed_line = line.trim();

                    if trimmed_line.starts_with("data: ") {
                        let json_str = trimmed_line.strip_prefix("data: ").unwrap_or("");

                        if json_str == "[DONE]" {
                            break;
                        }

                        if !json_str.is_empty() {
                            // Try to parse the JSON and extract content
                            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str)
                            {
                                if usage.is_none() {
                                    if let Some(collected) = parsed.get("usage") {
                                        if !collected.is_null() {
                                            usage = Some(collected.clone());
                                        }
                                    }
                                }
                                if let Some(choices) =
                                    parsed.get("choices").and_then(|c| c.as_array())
                                {
                                    if let Some(first_choice) = choices.first() {
                                        if let Some(delta) = first_choice.get("delta") {
                                            if let Some(content) =
                                                delta.get("content").and_then(|c| c.as_str())
                                            {
                                                full_response.push_str(content);
                                                // Emit just the content to frontend
                                                let _ = app.emit("chat_stream_chunk", content);
                                                stream_started = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Update buffer with incomplete line
                buffer = incomplete_line;
            }
            Err(e) => {
                let sources = vec![e.to_string()];
                let final_message = map_api_error_message(&error_rules, &sources);
                tauri::async_runtime::spawn({
                    let app = app.clone();
                    let provider = provider.clone();
                    let model = model.clone();
                    let error_msg = e.to_string();
                    async move {
                        report_api_error(app, error_msg, "/api/chat".to_string(), model, provider)
                            .await;
                    }
                });
                return Err(final_message);
            }
        }
    }

    // Emit completion event
    let _ = app.emit("chat_stream_complete", &full_response);

    if stream_started && !full_response.is_empty() {
        tauri::async_runtime::spawn({
            let activity_app = app.clone();
            let activity_model = api_config.model.clone();
            let activity_app_version = app.package_info().version.to_string();
            let captured_metrics = usage.clone();
            async move {
                let _ = user_activity(
                    activity_app,
                    captured_metrics,
                    activity_model,
                    activity_app_version,
                )
                .await;
            }
        });
    }

    Ok(full_response)
}

async fn user_activity(
    app: AppHandle,
    activity_metrics: Option<serde_json::Value>,
    configured_model: String,
    app_version: String,
) -> Result<(), String> {
    let app_endpoint = match get_app_endpoint() {
        Ok(value) => value,
        Err(_) => return Ok(()),
    };

    let api_access_key = match get_api_access_key() {
        Ok(value) => value,
        Err(_) => return Ok(()),
    };

    let (license_key, instance_id, stored_model) = match get_stored_credentials(&app).await {
        Ok(values) => values,
        Err(_) => return Ok(()),
    };

    let machine_id = "anonymous-open-source-machine".to_string();

    if machine_id.is_empty() {
        return Ok(());
    }

    let ai_model = stored_model
        .as_ref()
        .map(|model| model.model.clone())
        .unwrap_or(configured_model);

    let mut payload = serde_json::json!({
        "license": license_key,
        "instance": instance_id,
        "machine_id": machine_id,
        "app_version": app_version,
        "ai_model": ai_model,
    });

    if let Some(metrics) = activity_metrics {
        if let Some(obj) = payload.as_object_mut() {
            const METRIC_FIELD_BYTES: [u8; 5] = [117, 115, 97, 103, 101];
            if let Ok(field) = std::str::from_utf8(&METRIC_FIELD_BYTES) {
                obj.insert(field.to_string(), metrics);
            }
        }
    }

    let activity_url = format!("{}/api/activity", app_endpoint.trim_end_matches('/'));
    let client = reqwest::Client::new();

    let _ = client
        .post(&activity_url)
        .header("Authorization", format!("Bearer {}", api_access_key))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await;

    Ok(())
}

async fn report_api_error(
    app: AppHandle,
    error_message: String,
    endpoint: String,
    model: Option<String>,
    provider: Option<String>,
) {
    let app_endpoint = match get_app_endpoint() {
        Ok(value) => value,
        Err(_) => return,
    };

    let api_access_key = match get_api_access_key() {
        Ok(value) => value,
        Err(_) => return,
    };

    let (license_key, instance_id, stored_model) = match get_stored_credentials(&app).await {
        Ok(values) => values,
        Err(_) => return,
    };

    let machine_id = "anonymous-open-source-machine".to_string();

    if machine_id.is_empty() {
        return;
    }

    let app_version = app.package_info().version.to_string();

    let final_model = model
        .or_else(|| stored_model.as_ref().map(|m| m.model.clone()))
        .unwrap_or_default();

    let final_provider = provider
        .or_else(|| stored_model.as_ref().map(|m| m.provider.clone()))
        .unwrap_or_default();

    let payload = serde_json::json!({
        "machine_id": machine_id,
        "error_message": error_message,
        "app_version": app_version,
        "instance": instance_id,
        "license_key": license_key,
        "endpoint": endpoint,
        "model": final_model,
        "provider": final_provider
    });

    let error_url = format!("{}/api/error", app_endpoint.trim_end_matches('/'));
    let client = reqwest::Client::new();

    tracing::debug!("Reporting API error: {:?}", payload);

    if let Err(e) = client
        .post(&error_url)
        .header("Authorization", format!("Bearer {}", api_access_key))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        tracing::warn!("Failed to report API error: {}", e);
    }
}

// Models API Command
#[tauri::command]
pub async fn fetch_models(app: AppHandle) -> Result<Vec<Model>, String> {
    // Get environment variables — return empty list gracefully if not configured
    let app_endpoint = match get_app_endpoint() {
        Ok(v) => v,
        Err(_) => return Ok(vec![]),
    };
    let api_access_key = match get_api_access_key() {
        Ok(v) => v,
        Err(_) => return Ok(vec![]),
    };

    let (license_key, instance_id) = match get_stored_credentials(&app).await {
        Ok((lk, id, _)) => (lk, id),
        Err(_) => ("".to_string(), "".to_string()),
    };
    let machine_id = "anonymous-open-source-machine".to_string();
    let app_version = app.package_info().version.to_string();

    let client = reqwest::Client::new();
    let url = format!("{}/api/models", app_endpoint);

    let response = match client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_access_key))
        .header("license_key", &license_key)
        .header("instance", &instance_id)
        .header("machine_id", &machine_id)
        .header("app_version", &app_version)
        .send()
        .await
    {
        Ok(resp) => resp,
        // Network error — return empty list gracefully
        Err(_) => return Ok(vec![]),
    };

    // If server returns non-2xx (e.g. 404 from marketing site), return empty gracefully
    if !response.status().is_success() {
        return Ok(vec![]);
    }

    let models_response: ModelsResponse = match response.json().await {
        Ok(m) => m,
        Err(_) => return Ok(vec![]),
    };

    Ok(models_response.models)
}

// Fetch Rapid Text Prompts API
#[tauri::command]
pub async fn fetch_prompts() -> Result<RapidTextPromptsResponse, String> {
    let app_endpoint = match get_app_endpoint() {
        Ok(v) => v,
        Err(_) => {
            return Ok(RapidTextPromptsResponse {
                prompts: vec![],
                total: 0,
                last_updated: None,
            })
        }
    };
    let api_access_key = match get_api_access_key() {
        Ok(v) => v,
        Err(_) => {
            return Ok(RapidTextPromptsResponse {
                prompts: vec![],
                total: 0,
                last_updated: None,
            })
        }
    };

    let client = reqwest::Client::new();
    let url = format!("{}/api/prompts", app_endpoint);

    let response = match client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_access_key))
        .send()
        .await
    {
        Ok(resp) => resp,
        // Network error — return empty list gracefully
        Err(_) => {
            return Ok(RapidTextPromptsResponse {
                prompts: vec![],
                total: 0,
                last_updated: None,
            })
        }
    };

    // If server returns non-2xx (e.g. 404 from marketing site), return empty gracefully
    if !response.status().is_success() {
        return Ok(RapidTextPromptsResponse {
            prompts: vec![],
            total: 0,
            last_updated: None,
        });
    }

    let prompts_response: RapidTextPromptsResponse = match response.json().await {
        Ok(p) => p,
        Err(_) => {
            return Ok(RapidTextPromptsResponse {
                prompts: vec![],
                total: 0,
                last_updated: None,
            })
        }
    };

    Ok(prompts_response)
}

// Create System Prompt API Command
#[tauri::command]
pub async fn create_system_prompt(
    app: AppHandle,
    user_prompt: String,
) -> Result<SystemPromptResponse, String> {
    // Get environment variables
    let app_endpoint = get_app_endpoint()?;
    let api_access_key = get_api_access_key()?;
    let (license_key, instance_id, _) = get_stored_credentials(&app).await?;
    let machine_id: String = "anonymous-open-source-machine".to_string();
    let app_version: String = app.package_info().version.to_string();
    // Make HTTP request to models endpoint
    let client = reqwest::Client::new();
    let url = format!("{}/api/prompt", app_endpoint);

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_access_key))
        .header("license_key", &license_key)
        .header("instance", &instance_id)
        .header("machine_id", &machine_id)
        .header("app_version", &app_version)
        .json(&serde_json::json!({
            "user_prompt": user_prompt
        }))
        .send()
        .await
        .map_err(|e| {
            let error_msg = format!("{}", e);
            if error_msg.contains("url (") {
                // Remove the URL part from the error message
                let parts: Vec<&str> = error_msg.split(" for url (").collect();
                if parts.len() > 1 {
                    format!("Failed to make models request: {}", parts[0])
                } else {
                    format!("Failed to make models request: {}", error_msg)
                }
            } else {
                format!("Failed to make models request: {}", error_msg)
            }
        })?;

    // Check if the response is successful
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown server error".to_string());

        // Try to parse error as JSON to get a more specific error message
        if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&error_text) {
            if let Some(error_msg) = error_json.get("error").and_then(|e| e.as_str()) {
                return Err(format!("Server error ({}): {}", status, error_msg));
            } else if let Some(message) = error_json.get("message").and_then(|m| m.as_str()) {
                return Err(format!("Server error ({}): {}", status, message));
            }
        }

        return Err(format!("Server error ({}): {}", status, error_text));
    }

    let system_prompt_response: SystemPromptResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse system prompt response: {}", e))?;

    Ok(system_prompt_response)
}

// Helper command to check if license is available
#[tauri::command]
pub async fn check_license_status(app: AppHandle) -> Result<bool, String> {
    match get_stored_credentials(&app).await {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[allow(dead_code)]
#[tauri::command]
pub async fn get_activity(app: AppHandle) -> Result<serde_json::Value, String> {
    let app_endpoint = get_app_endpoint()?;
    let api_access_key = get_api_access_key()?;

    let (license_key, instance_id, _) = get_stored_credentials(&app).await?;

    let machine_id = "anonymous-open-source-machine".to_string();

    if machine_id.is_empty() {
        return Err("Machine identifier unavailable".to_string());
    }

    let app_version = app.package_info().version.to_string();

    let client = reqwest::Client::new();
    let activity_url = format!("{}/api/activity", app_endpoint.trim_end_matches('/'));

    let response = client
        .get(&activity_url)
        .header("Authorization", format!("Bearer {}", api_access_key))
        .header("license_key", &license_key)
        .header("instance_name", &instance_id)
        .header("machine_id", machine_id)
        .header("app_version", app_version)
        .send()
        .await
        .map_err(|e| {
            let error_msg = format!("{}", e);
            if error_msg.contains("url (") {
                let parts: Vec<&str> = error_msg.split(" for url (").collect();
                if parts.len() > 1 {
                    format!("Failed to request activity: {}", parts[0])
                } else {
                    format!("Failed to request activity: {}", error_msg)
                }
            } else {
                format!("Failed to request activity: {}", error_msg)
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown server error".to_string());

        if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&error_text) {
            if let Some(message) = error_json
                .get("message")
                .and_then(|m| m.as_str())
                .or_else(|| error_json.get("error").and_then(|m| m.as_str()))
            {
                return Err(format!("Server error ({}): {}", status, message));
            }
        }

        return Err(format!("Server error ({}): {}", status, error_text));
    }

    response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("Failed to parse activity response: {}", e))
}

pub async fn save_voice_note(
    app: &tauri::AppHandle,
    raw_transcription: &str,
    content: &str,
) -> Result<(), String> {
    use sqlx::{Connection, SqliteConnection};
    use uuid::Uuid;

    let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db_path = app_data.join("rapidtext.db");
    
    let db_url = format!("sqlite:{}", db_path.to_string_lossy());
    let mut conn = SqliteConnection::connect(&db_url)
        .await
        .map_err(|e| format!("Failed to connect to SQLite: {}", e))?;
        
    let id = Uuid::new_v4().to_string();
    
    let title = if content.len() > 40 {
        format!("{}...", &content[..40])
    } else if !content.trim().is_empty() {
        content.to_string()
    } else {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        format!("Voice Note {}", now)
    };
    
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    sqlx::query(
        "INSERT INTO voice_notes (id, title, content, raw_transcription, created_at, updated_at, is_pinned) VALUES ($1, $2, $3, $4, $5, $6, 0)"
    )
    .bind(id)
    .bind(title)
    .bind(content)
    .bind(raw_transcription)
    .bind(now_ms)
    .bind(now_ms)
    .execute(&mut conn)
    .await
    .map_err(|e| format!("Failed to insert voice note: {}", e))?;

    Ok(())
}
