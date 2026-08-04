use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, PhysicalSize, PhysicalPosition};
use crate::dictation_state::DictationConfigState;
use std::sync::{Arc, Mutex};

/// Set up the main pill window's position based on resting_position configuration
pub fn setup_main_window<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(window) = app.get_webview_window("main") {
        let config_state = app.state::<Arc<Mutex<DictationConfigState>>>();
        let position_str = if let Ok(guard) = config_state.lock() {
            guard.resting_position.clone()
        } else {
            "bottom_right".to_string()
        };

        if let Ok(Some(monitor)) = window.primary_monitor() {
            let monitor_size = monitor.size();
            let monitor_pos = monitor.position();
            let win_width = 54;
            let win_height = 54;
            
            // Calculate screen coordinates based on resting position
            let (x, y) = match position_str.as_str() {
                "top_center" => (
                    monitor_pos.x + ((monitor_size.width as i32 - win_width) / 2),
                    monitor_pos.y + 50,
                ),
                "top_left" => (
                    monitor_pos.x + 50,
                    monitor_pos.y + 50,
                ),
                "top_right" => (
                    monitor_pos.x + (monitor_size.width as i32 - win_width - 50),
                    monitor_pos.y + 50,
                ),
                "bottom_left" => (
                    monitor_pos.x + 50,
                    monitor_pos.y + (monitor_size.height as i32 - win_height - 50),
                ),
                "bottom_right" => (
                    monitor_pos.x + (monitor_size.width as i32 - win_width - 50),
                    monitor_pos.y + (monitor_size.height as i32 - win_height - 50),
                ),
                _ => ( // default to bottom_center
                    monitor_pos.x + ((monitor_size.width as i32 - win_width) / 2),
                    monitor_pos.y + (monitor_size.height as i32 - win_height - 50),
                ),
            };
            
            let _ = window.set_size(tauri::Size::Physical(PhysicalSize::new(win_width as u32, win_height as u32)));
            let _ = window.set_position(tauri::Position::Physical(PhysicalPosition::new(x, y)));
            let _ = window.set_always_on_top(true);
        }
        let _ = window.show();
    }
    Ok(())
}

/// Dynamically create the dashboard/settings window when clicked
#[tauri::command]
pub fn open_dashboard(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("dashboard") {
        let _ = win.show();
        let _ = win.set_focus();
        return Ok(());
    }

    // Build secondary dashboard window dynamically on demand
    let dashboard_win = WebviewWindowBuilder::new(
        &app,
        "dashboard",
        WebviewUrl::App("index.html".into())
    )
    .title("Rapid Text Dashboard")
    .inner_size(400.0, 600.0)
    .resizable(false)
    .decorations(true)
    .always_on_top(false)
    .visible(false);

    let win = dashboard_win.build().map_err(|e| e.to_string())?;

    // Position above the pill window if visible
    if let Some(main_win) = app.get_webview_window("main") {
        if let Ok(main_pos) = main_win.outer_position() {
            if let Ok(main_size) = main_win.outer_size() {
                let x = main_pos.x + ((main_size.width as i32 - 400) / 2);
                let y = main_pos.y - 620; // 600px height + 20px spacer
                let _ = win.set_position(tauri::Position::Physical(PhysicalPosition::new(x, y)));
            }
        }
    }

    let _ = win.show();
    let _ = win.set_focus();
    Ok(())
}
