// Force GUI subsystem on Windows so Windows NEVER allocates a console window on startup or launch
#![windows_subsystem = "windows"]

#[cfg(target_os = "windows")]
fn hide_console() {
    use windows_sys::Win32::System::Console::{FreeConsole, GetConsoleWindow};
    use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
    
    let hwnd = unsafe { GetConsoleWindow() };
    if !hwnd.is_null() {
        unsafe {
            ShowWindow(hwnd, SW_HIDE);
            FreeConsole();
        }
    }
}

fn main() {
    #[cfg(target_os = "windows")]
    hide_console();

    rapid_text_lib::run()
}
