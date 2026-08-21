use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::AppHandle;

use super::{overlay_dimensions, windows_overlay_bounds, OVERLAY_HEIGHT, OVERLAY_WIDTH};

static LAST_MIC_LEVEL_EMIT: AtomicU64 = AtomicU64::new(0);
const EMIT_THROTTLE_MS: u64 = 16;

// Store last state so update_overlay_position can re-place.
static LAST_STATE: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
// Store last levels for rendering (9 bars).
static LAST_LEVELS: std::sync::Mutex<Vec<f32>> = std::sync::Mutex::new(Vec::new());

// HWND handle — OnceLock because we create once.
static HWND: std::sync::OnceLock<isize> = std::sync::OnceLock::new();

#[cfg(target_os = "windows")]
fn get_hwnd() -> Option<windows::Win32::Foundation::HWND> {
    HWND.get()
        .map(|&raw| windows::Win32::Foundation::HWND(raw as *mut _))
}

#[cfg(target_os = "windows")]
fn ensure_window(app_handle: &AppHandle) -> Option<windows::Win32::Foundation::HWND> {
    if let Some(hwnd) = get_hwnd() {
        return Some(hwnd);
    }
    // Create on main thread synchronously.
    let handle = app_handle.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = handle.run_on_main_thread(move || {
        let hwnd = create_hwnd();
        let raw = hwnd.0 as isize;
        let _ = HWND.set(raw);
        let _ = tx.send(hwnd);
    });
    rx.recv().ok()
}

#[cfg(target_os = "windows")]
fn create_hwnd() -> windows::Win32::Foundation::HWND {
    use windows::core::w;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::HBRUSH;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, RegisterClassExW, CS_HREDRAW, CS_VREDRAW, HMENU, WNDCLASSEXW,
        WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
        WS_POPUP,
    };

    unsafe extern "system" fn wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: windows::Win32::Foundation::WPARAM,
        lparam: windows::Win32::Foundation::LPARAM,
    ) -> windows::Win32::Foundation::LRESULT {
        use windows::Win32::UI::WindowsAndMessaging::{
            DefWindowProcW, HTCLIENT, HTTRANSPARENT, WM_NCHITTEST, WM_PAINT,
        };
        match msg {
            WM_NCHITTEST => windows::Win32::Foundation::LRESULT(HTTRANSPARENT as isize),
            WM_PAINT => {
                // Minimal paint: fill with layered alpha via tiny-skia/softbuffer later.
                // For v1, just validate via DefWindowProc to keep window transparent.
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    unsafe {
        let class_name = w!("HandyOverlayNative");
        let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap();

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            hbrBackground: HBRUSH(std::ptr::null_mut()),
            ..Default::default()
        };
        RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE,
            class_name,
            w!("Handy"),
            WS_POPUP,
            0,
            0,
            (OVERLAY_WIDTH as i32),
            (OVERLAY_HEIGHT as i32),
            None,
            Some(HMENU(std::ptr::null_mut())),
            Some(hinstance.into()),
            None,
        )
        .unwrap_or(HWND(std::ptr::null_mut()));

        // Make window layered with full alpha (opaque pill draws itself).
        // LWA_ALPHA 0x2 — 255 = opaque.
        windows::Win32::UI::WindowsAndMessaging::SetLayeredWindowAttributes(
            hwnd,
            windows::Win32::Foundation::COLORREF(0),
            255,
            windows::Win32::UI::WindowsAndMessaging::LWA_ALPHA,
        )
        .ok();

        hwnd
    }
}

#[cfg(target_os = "windows")]
fn show_state(app_handle: &AppHandle, state: &str) {
    if crate::settings::get_settings(app_handle).overlay_style
        == crate::settings::OverlayStyle::None
    {
        return;
    }
    {
        let mut last = LAST_STATE.lock().unwrap();
        *last = state.to_string();
    }
    let (lw, lh) = overlay_dimensions(state);
    let handle = app_handle.clone();
    let state_owned = state.to_string();
    let _ = app_handle.run_on_main_thread(move || {
        let Some(monitor) = super::get_monitor_with_cursor(&handle) else {
            log::debug!("native windows overlay: no monitor for cursor");
            return;
        };
        let (x, y, w, h) = windows_overlay_bounds(
            *monitor.position(),
            *monitor.size(),
            monitor.scale_factor(),
            lw,
            lh,
            crate::settings::get_settings(&handle).overlay_position,
        );
        let Some(hwnd) = ensure_window(&handle) else {
            log::error!("native windows overlay: failed to ensure HWND");
            return;
        };
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{
                SetWindowPos, ShowWindow, HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW,
                SW_SHOWNOACTIVATE,
            };
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                x,
                y,
                w,
                h,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            // Ensure topmost after show.
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE
                    | windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE
                    | SWP_NOACTIVATE
                    | SWP_SHOWWINDOW,
            );
        }
        log::debug!(
            "native windows overlay '{}' at {} {} {} {} scale {}",
            state_owned,
            x,
            y,
            w,
            h,
            monitor.scale_factor()
        );
        // TODO: softbuffer/tiny-skia render pill here via window surface.
        let _ = &state_owned;
    });
}

pub fn create_recording_overlay(_app_handle: &AppHandle) {
    // Lazy creation on first show.
    log::debug!("native windows overlay: create (lazy)");
}

pub fn show_recording_overlay(app_handle: &AppHandle) {
    show_state(app_handle, "recording");
}

pub fn show_streaming_overlay(app_handle: &AppHandle) {
    show_state(app_handle, "streaming");
}

pub fn show_transcribing_overlay(app_handle: &AppHandle) {
    show_state(app_handle, "transcribing");
}

pub fn show_processing_overlay(app_handle: &AppHandle) {
    show_state(app_handle, "processing");
}

pub fn update_overlay_position(app_handle: &AppHandle) {
    let state = LAST_STATE.lock().unwrap().clone();
    if state.is_empty() {
        return;
    }
    show_state(app_handle, &state);
}

pub fn hide_recording_overlay(app_handle: &AppHandle) {
    let handle = app_handle.clone();
    let _ = handle.run_on_main_thread(move || {
        if let Some(hwnd) = get_hwnd() {
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
                    hwnd,
                    windows::Win32::UI::WindowsAndMessaging::SW_HIDE,
                );
            }
        }
    });
}

pub fn emit_levels(_app_handle: &AppHandle, levels: &[f32]) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let last = LAST_MIC_LEVEL_EMIT.load(Ordering::Relaxed);
    if now.saturating_sub(last) < EMIT_THROTTLE_MS {
        return;
    }
    LAST_MIC_LEVEL_EMIT.store(now, Ordering::Relaxed);

    {
        let mut guard = LAST_LEVELS.lock().unwrap();
        *guard = levels.to_vec();
    }
    // In a full implementation, invalidate the HWND to trigger WM_PAINT with softbuffer.
    // For v1 stub, just store levels.
    #[cfg(target_os = "windows")]
    if let Some(hwnd) = get_hwnd() {
        unsafe {
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(Some(hwnd), None, false);
        }
    }
}
