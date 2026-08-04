// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod api;
mod db;
mod dictation_state;
mod shortcut;
mod pill_window;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, WebviewWindow};
use tokio::task::JoinHandle;
mod speaker;
use dictation_state::DictationConfigState;
use speaker::VadConfig;

#[cfg(target_os = "macos")]
#[allow(deprecated)]
use tauri_nspanel::{cocoa::appkit::NSWindowCollectionBehavior, panel_delegate, WebviewWindowExt};

#[derive(Default)]
pub struct AudioState {
    stream_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    vad_config: Arc<Mutex<VadConfig>>,
    is_capturing: Arc<Mutex<bool>>,
    is_starting: Arc<std::sync::atomic::AtomicBool>,
    pub active_streams_count: Arc<std::sync::atomic::AtomicUsize>,
    pub stt_config: Arc<Mutex<Option<api::ActiveSttConfig>>>,
    pub stt_key_index: Arc<std::sync::atomic::AtomicUsize>,
    pub active_capture_mode: Arc<Mutex<Option<String>>>,
}

#[tauri::command]
fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
async fn toggle_rapid_text_recording(app: AppHandle) -> Result<(), String> {
    shortcut::toggle_recording(&app).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "windows")]
    {
        if std::env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").is_err() {
            std::env::set_var(
                "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
                "--disable-background-networking --disable-component-update",
            );
        }
    }

    let mut builder = tauri::Builder::default()
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations("sqlite:rapidtext.db", db::migrations())
                .build(),
        )
        .manage(AudioState::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_keychain::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _new_cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_shell::init()); // Add shell plugin
    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }
    let builder = builder
        .invoke_handler(tauri::generate_handler![
            get_app_version,
            pill_window::open_dashboard,
            api::sync_dictation_config_to_rust,
            api::get_dictation_config,
            api::update_active_stt_config,
            toggle_rapid_text_recording,
            speaker::start_system_audio_capture,
            speaker::stop_system_audio_capture,
            speaker::manual_stop_continuous,
            speaker::check_system_audio_access,
            speaker::request_system_audio_access,
            speaker::get_vad_config,
            speaker::update_vad_config,
            speaker::get_capture_status,
            speaker::get_audio_sample_rate,
            speaker::get_input_devices,
            speaker::get_output_devices,
        ])
        .setup(|app| {
            // Load native configuration cache directly from JSON for instant startup
            let config_state = if let Ok(app_data) = app.path().app_data_dir() {
                let config_path = app_data.join("dictation_config.json");
                if let Ok(content) = std::fs::read_to_string(config_path) {
                    serde_json::from_str(&content).unwrap_or_default()
                } else {
                    DictationConfigState::default()
                }
            } else {
                DictationConfigState::default()
            };
            app.manage(Arc::new(Mutex::new(config_state)));

            // Pre-warm connections to Groq and OpenAI to bypass TCP/TLS handshake latency (approx 350ms saved)
            let client = reqwest::Client::new();
            tauri::async_runtime::spawn(async move {
                let _ = client.head("https://api.groq.com/openai/v1/audio/transcriptions").send().await;
                let _ = client.head("https://api.openai.com/v1/audio/transcriptions").send().await;
            });

            // Set up main window positioning & appearance
            pill_window::setup_main_window(app.handle()).expect("Failed to setup main window");
            
            if let Some(window) = app.get_webview_window("main") {
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { .. } = event {
                        std::process::exit(0);
                    }
                });
            }
            #[cfg(target_os = "macos")]
            init(app.app_handle());

            // Initialize global shortcut plugin with static handler (Alt+D = dictation, Alt+P = exit)
            app.handle()
                .plugin(
                    tauri_plugin_global_shortcut::Builder::new()
                        .with_handler(move |app, shortcut, event| {
                            use tauri_plugin_global_shortcut::{Shortcut, ShortcutState};

                            if event.state() == ShortcutState::Pressed {
                                if let Ok(dictation_shortcut) = "Alt+D".parse::<Shortcut>() {
                                    if shortcut == &dictation_shortcut {
                                        let app_handle = app.clone();
                                        tauri::async_runtime::spawn(async move {
                                            if let Err(e) = crate::shortcut::toggle_recording(&app_handle).await {
                                                eprintln!("Failed to toggle recording from hotkey: {}", e);
                                            }
                                        });
                                    }
                                }
                                
                                if let Ok(kill_shortcut) = "Alt+P".parse::<Shortcut>() {
                                    if shortcut == &kill_shortcut {
                                        std::process::exit(0);
                                    }
                                }
                            }
                        })
                        .build(),
                )
                .expect("Failed to initialize global shortcut plugin");

            // Register global shortcuts
            if let Err(e) = shortcut::setup_global_shortcut(app.handle()) {
                eprintln!("Failed to setup global shortcuts: {}", e);
            }
            Ok(())
        });

    // Add macOS-specific permissions plugin
    #[cfg(target_os = "macos")]
    let builder = {
        builder.plugin(tauri_plugin_macos_permissions::init())
    };

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(target_os = "macos")]
#[allow(deprecated, unexpected_cfgs)]
fn init(app_handle: &AppHandle) {
    let window: WebviewWindow = app_handle.get_webview_window("main").unwrap();

    let panel = window.to_panel().unwrap();

    let delegate = panel_delegate!(MyPanelDelegate {
        window_did_become_key,
        window_did_resign_key
    });

    let handle = app_handle.to_owned();

    delegate.set_listener(Box::new(move |delegate_name: String| {
        match delegate_name.as_str() {
            "window_did_become_key" => {
                let app_name = handle.package_info().name.to_owned();

                println!("[info]: {:?} panel becomes key window!", app_name);
            }
            "window_did_resign_key" => {
                println!("[info]: panel resigned from key window!");
            }
            _ => (),
        }
    }));

    // Set the window to float level
    #[allow(non_upper_case_globals)]
    const NSFloatWindowLevel: i32 = 4;
    panel.set_level(NSFloatWindowLevel);

    #[allow(non_upper_case_globals)]
    const NSWindowStyleMaskNonActivatingPanel: i32 = 1 << 7;
    panel.set_style_mask(NSWindowStyleMaskNonActivatingPanel);

    #[allow(deprecated)]
    panel.set_collection_behaviour(
        NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces,
    );

    panel.set_delegate(delegate);
}

#[cfg(debug_assertions)]
pub(crate) fn write_debug_log(message: String) {
    use std::fs::OpenOptions;
    use std::io::Write;
    let log_path = std::env::temp_dir().join("rapid_text_debug.log");
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(log_path)
    {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let _ = writeln!(file, "[{}] [RUST] {}", timestamp, message);
    }
}

#[cfg(not(debug_assertions))]
pub(crate) fn write_debug_log(_message: String) {
    // No-op in release builds to eliminate disk I/O overhead
}
