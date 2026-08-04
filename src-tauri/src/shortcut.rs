use tauri::{AppHandle, Manager, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use crate::speaker::CaptureMode;

/// Setup static global shortcuts: Alt+D (dictation), Alt+P (absolute exit)
pub fn setup_global_shortcut(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let shortcut_manager = app.global_shortcut();
    
    if let Ok(dictation_shortcut) = "Alt+D".parse::<Shortcut>() {
        let _ = shortcut_manager.register(dictation_shortcut);
    }
    
    if let Ok(kill_shortcut) = "Alt+P".parse::<Shortcut>() {
        let _ = shortcut_manager.register(kill_shortcut);
    }

    Ok(())
}

/// Toggle recording state in Rust (Start/Stop CPAL native capture)
pub async fn toggle_recording(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<crate::AudioState>();
    
    // Check current capturing status
    let is_capturing = if let Ok(guard) = state.is_capturing.lock() {
        *guard
    } else {
        false
    };

    if is_capturing {
        // Stop audio capture
        crate::speaker::stop_system_audio_capture_impl(app.clone()).await?;
        let _ = app.emit("capture-stopped", ());
    } else {
        // Start audio capture in Dictation mode
        crate::speaker::start_system_audio_capture(
            app.clone(),
            None,
            None,
            None,
            Some(CaptureMode::Dictation),
        ).await?;
        let _ = app.emit("capture-started", "dictation");
    }

    Ok(())
}
