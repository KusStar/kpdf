#[cfg(target_os = "windows")]
pub(super) fn restore_window_native(window: &Window) -> bool {
    let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(window) else {
        return false;
    };

    let RawWindowHandle::Win32(win32) = handle.as_raw() else {
        return false;
    };

    // raw-window-handle guarantees non-zero HWND for Win32 handles.
    let hwnd = HWND(win32.hwnd.get() as _);
    unsafe { ShowWindowAsync(hwnd, SW_RESTORE).as_bool() }
}

pub(super) fn zoom_or_restore_window(window: &Window) {
    #[cfg(target_os = "windows")]
    {
        if window.is_maximized() && restore_window_native(window) {
            return;
        }
    }

    window.zoom_window();
}
