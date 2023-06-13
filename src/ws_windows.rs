#[cfg(target_os = "windows")]
use winapi::shared::windef::HWND;
#[cfg(target_os = "windows")]
use winapi::um::winuser::{IsIconic, IsWindowVisible, IsWindow};

pub enum WindowStatus{
    Visible,
    Destroyed,
    Minimized,
}
#[cfg(target_os = "windows")]
pub fn hwnd_exists(hwnd: isize) -> WindowStatus {
    let hwnd = hwnd as HWND;

    // Call IsWindow to determine if the window still exists
    let exists = unsafe { IsWindow(hwnd) != 0 };
    if !exists {
        return WindowStatus::Destroyed;
    }

    // Check if the window is visible and not minimized
    let visible = unsafe { IsWindowVisible(hwnd) != 0 };
    let minimized = unsafe { IsIconic(hwnd) != 0 };
    if !visible || minimized {
        return WindowStatus::Minimized;
    }

    WindowStatus::Visible
}
