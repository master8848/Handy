use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, PhysicalPosition, PhysicalSize};

use crate::settings;

// ---------------------------------------------------------------------------
// Shared geometry — single source of truth for both native and WebView paths.
// Keep these in sync with RecordingOverlay.css `--ov-*` vars.
// ---------------------------------------------------------------------------

pub const OVERLAY_WIDTH: f64 = 256.0;
pub const OVERLAY_HEIGHT: f64 = 46.0;
pub const OVERLAY_STREAM_WIDTH: f64 = 400.0;
pub const OVERLAY_STREAM_HEIGHT: f64 = 120.0;

/// Overlay window size (logical) for a given UI state.
pub fn overlay_dimensions(state: &str) -> (f64, f64) {
    if state == "streaming" {
        (OVERLAY_STREAM_WIDTH, OVERLAY_STREAM_HEIGHT)
    } else {
        (OVERLAY_WIDTH, OVERLAY_HEIGHT)
    }
}

#[cfg(target_os = "macos")]
pub const OVERLAY_TOP_OFFSET: f64 = 46.0;
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub const OVERLAY_TOP_OFFSET: f64 = 4.0;

#[cfg(target_os = "macos")]
pub const OVERLAY_BOTTOM_OFFSET: f64 = 15.0;
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub const OVERLAY_BOTTOM_OFFSET: f64 = 40.0;

// ---------------------------------------------------------------------------
// Monitor helpers — reused by WebView and native (macOS NSPanel.setFrameOrigin
// and Windows HWND SetWindowPos both need the same monitor math).
// ---------------------------------------------------------------------------

pub fn get_monitor_with_cursor(app_handle: &AppHandle) -> Option<tauri::Monitor> {
    if let Some(mouse_location) = crate::input::get_cursor_position(app_handle) {
        if let Ok(monitors) = app_handle.available_monitors() {
            for monitor in monitors {
                #[cfg(target_os = "windows")]
                if is_mouse_within_monitor(mouse_location, monitor.position(), monitor.size()) {
                    return Some(monitor);
                }

                #[cfg(not(target_os = "windows"))]
                {
                    let scale = monitor.scale_factor();
                    let pos = PhysicalPosition::new(
                        (monitor.position().x as f64 / scale) as i32,
                        (monitor.position().y as f64 / scale) as i32,
                    );
                    let size = PhysicalSize::new(
                        (monitor.size().width as f64 / scale) as u32,
                        (monitor.size().height as f64 / scale) as u32,
                    );
                    if is_mouse_within_monitor(mouse_location, &pos, &size) {
                        return Some(monitor);
                    }
                }
            }
        }
    }

    app_handle.primary_monitor().ok().flatten()
}

pub fn is_mouse_within_monitor(
    mouse_pos: (i32, i32),
    monitor_pos: &PhysicalPosition<i32>,
    monitor_size: &PhysicalSize<u32>,
) -> bool {
    let (mouse_x, mouse_y) = mouse_pos;
    let PhysicalPosition {
        x: monitor_x,
        y: monitor_y,
    } = *monitor_pos;
    let PhysicalSize {
        width: monitor_width,
        height: monitor_height,
    } = *monitor_size;

    mouse_x >= monitor_x
        && mouse_x < (monitor_x + monitor_width as i32)
        && mouse_y >= monitor_y
        && mouse_y < (monitor_y + monitor_height as i32)
}

/// Returns overlay position in logical coordinates (points on macOS).
pub fn calculate_overlay_position(
    app_handle: &AppHandle,
    width: f64,
    height: f64,
) -> Option<(f64, f64)> {
    let monitor = get_monitor_with_cursor(app_handle)?;
    let scale = monitor.scale_factor();
    let monitor_x = monitor.position().x as f64 / scale;
    let monitor_y = monitor.position().y as f64 / scale;
    let monitor_width = monitor.size().width as f64 / scale;

    let settings = settings::get_settings(app_handle);

    let x = monitor_x + (monitor_width - width) / 2.0;
    let y = match settings.overlay_position {
        crate::settings::OverlayPosition::Top => monitor_y + OVERLAY_TOP_OFFSET,
        crate::settings::OverlayPosition::Bottom => {
            #[cfg(target_os = "macos")]
            let bottom = {
                let wa = monitor.work_area();
                (wa.position.y as f64 + wa.size.height as f64) / scale
            };
            #[cfg(not(target_os = "macos"))]
            let bottom = monitor_y + monitor.size().height as f64 / scale;

            bottom - height - OVERLAY_BOTTOM_OFFSET
        }
    };

    Some((x, y))
}

/// Overlay rectangle in the destination monitor's physical pixels.
#[cfg(target_os = "windows")]
pub fn windows_overlay_bounds(
    monitor_position: PhysicalPosition<i32>,
    monitor_size: PhysicalSize<u32>,
    scale: f64,
    logical_width: f64,
    logical_height: f64,
    overlay_position: crate::settings::OverlayPosition,
) -> (i32, i32, i32, i32) {
    let width = (logical_width * scale).round().max(1.0) as i32;
    let height = (logical_height * scale).round().max(1.0) as i32;
    let x = (monitor_position.x as f64 + (monitor_size.width as f64 - width as f64) / 2.0).round()
        as i32;
    let y = match overlay_position {
        crate::settings::OverlayPosition::Top => {
            (monitor_position.y as f64 + OVERLAY_TOP_OFFSET * scale).round() as i32
        }
        crate::settings::OverlayPosition::Bottom => (monitor_position.y as f64
            + monitor_size.height as f64
            - height as f64
            - OVERLAY_BOTTOM_OFFSET * scale)
            .round() as i32,
    };

    (x, y, width, height)
}

// ---------------------------------------------------------------------------
// Cached "overlay is enabled" flag — kept in sync with overlay_style.
// ---------------------------------------------------------------------------

static OVERLAY_ENABLED: AtomicBool = AtomicBool::new(false);

/// Update the cached overlay-enabled flag. Called from `lib.rs` at startup and
/// from `change_overlay_style_setting` whenever the user changes visibility.
pub fn update_overlay_enabled_cache(enabled: bool) {
    OVERLAY_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn is_overlay_enabled_cached() -> bool {
    OVERLAY_ENABLED.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// Native gate
// ---------------------------------------------------------------------------

fn is_native_enabled(app: &AppHandle) -> bool {
    // Linux never uses native regardless of the flag (no HWND/NSPanel path).
    #[cfg(target_os = "linux")]
    {
        let _ = app;
        return false;
    }
    #[cfg(not(target_os = "linux"))]
    {
        settings::get_settings(app).overlay_native_enabled
    }
}

// ---------------------------------------------------------------------------
// Sub-modules
// ---------------------------------------------------------------------------

pub mod webview;

#[cfg(target_os = "macos")]
pub mod native_macos;

#[cfg(target_os = "windows")]
pub mod native_windows;

// ---------------------------------------------------------------------------
// Façade — preserve the 7+ public symbols so
// `lib.rs:354`, `utils.rs:76`, `managers/audio.rs:292`, `shortcut/mod.rs:681`
// stay unchanged.
// ---------------------------------------------------------------------------

pub fn create_recording_overlay(app_handle: &AppHandle) {
    if is_native_enabled(app_handle) {
        #[cfg(target_os = "macos")]
        {
            return native_macos::create_recording_overlay(app_handle);
        }
        #[cfg(target_os = "windows")]
        {
            return native_windows::create_recording_overlay(app_handle);
        }
        #[cfg(target_os = "linux")]
        {
            return webview::create_recording_overlay(app_handle);
        }
    }
    webview::create_recording_overlay(app_handle)
}

pub fn show_recording_overlay(app_handle: &AppHandle) {
    if is_native_enabled(app_handle) {
        #[cfg(target_os = "macos")]
        {
            return native_macos::show_recording_overlay(app_handle);
        }
        #[cfg(target_os = "windows")]
        {
            return native_windows::show_recording_overlay(app_handle);
        }
    }
    webview::show_recording_overlay(app_handle)
}

pub fn show_streaming_overlay(app_handle: &AppHandle) {
    if is_native_enabled(app_handle) {
        #[cfg(target_os = "macos")]
        {
            return native_macos::show_streaming_overlay(app_handle);
        }
        #[cfg(target_os = "windows")]
        {
            return native_windows::show_streaming_overlay(app_handle);
        }
    }
    webview::show_streaming_overlay(app_handle)
}

pub fn show_transcribing_overlay(app_handle: &AppHandle) {
    if is_native_enabled(app_handle) {
        #[cfg(target_os = "macos")]
        {
            return native_macos::show_transcribing_overlay(app_handle);
        }
        #[cfg(target_os = "windows")]
        {
            return native_windows::show_transcribing_overlay(app_handle);
        }
    }
    webview::show_transcribing_overlay(app_handle)
}

pub fn show_processing_overlay(app_handle: &AppHandle) {
    if is_native_enabled(app_handle) {
        #[cfg(target_os = "macos")]
        {
            return native_macos::show_processing_overlay(app_handle);
        }
        #[cfg(target_os = "windows")]
        {
            return native_windows::show_processing_overlay(app_handle);
        }
    }
    webview::show_processing_overlay(app_handle)
}

pub fn update_overlay_position(app_handle: &AppHandle) {
    if is_native_enabled(app_handle) {
        #[cfg(target_os = "macos")]
        {
            return native_macos::update_overlay_position(app_handle);
        }
        #[cfg(target_os = "windows")]
        {
            return native_windows::update_overlay_position(app_handle);
        }
    }
    webview::update_overlay_position(app_handle)
}

pub fn hide_recording_overlay(app_handle: &AppHandle) {
    if is_native_enabled(app_handle) {
        #[cfg(target_os = "macos")]
        {
            return native_macos::hide_recording_overlay(app_handle);
        }
        #[cfg(target_os = "windows")]
        {
            return native_windows::hide_recording_overlay(app_handle);
        }
    }
    webview::hide_recording_overlay(app_handle)
}

pub fn emit_levels(app_handle: &AppHandle, levels: &[f32]) {
    if !is_overlay_enabled_cached() {
        return;
    }
    if is_native_enabled(app_handle) {
        #[cfg(target_os = "macos")]
        {
            return native_macos::emit_levels(app_handle, levels);
        }
        #[cfg(target_os = "windows")]
        {
            return native_windows::emit_levels(app_handle, levels);
        }
    }
    webview::emit_levels(app_handle, levels)
}

// ---------------------------------------------------------------------------
// Tests — keep geometry tests here so they survive the split.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::{PhysicalPosition, PhysicalSize};

    #[test]
    fn monitor_hit_test_uses_half_open_physical_bounds() {
        let position = PhysicalPosition::new(-2560, -200);
        let size = PhysicalSize::new(2560, 1440);

        assert!(is_mouse_within_monitor((-2560, -200), &position, &size));
        assert!(is_mouse_within_monitor((-1, 1239), &position, &size));
        assert!(!is_mouse_within_monitor((0, 0), &position, &size));
        assert!(!is_mouse_within_monitor((-1, 1240), &position, &size));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_cursor_hit_test_does_not_scale_physical_monitor_bounds() {
        let position = PhysicalPosition::new(1920, 0);
        let size = PhysicalSize::new(3840, 2160);
        let cursor = (5000, 1000);

        assert!(is_mouse_within_monitor(cursor, &position, &size));

        let scale = 1.5;
        let logical_position = PhysicalPosition::new(
            (position.x as f64 / scale) as i32,
            (position.y as f64 / scale) as i32,
        );
        let logical_size = PhysicalSize::new(
            (size.width as f64 / scale) as u32,
            (size.height as f64 / scale) as u32,
        );
        assert!(!is_mouse_within_monitor(
            cursor,
            &logical_position,
            &logical_size
        ));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_overlay_bounds_use_destination_monitor_scale() {
        let monitor_position = PhysicalPosition::new(1920, 0);
        let monitor_size = PhysicalSize::new(3840, 2160);

        assert_eq!(
            windows_overlay_bounds(
                monitor_position,
                monitor_size,
                1.5,
                OVERLAY_WIDTH,
                OVERLAY_HEIGHT,
                crate::settings::OverlayPosition::Bottom,
            ),
            (3648, 2031, 384, 69)
        );
        assert_eq!(
            windows_overlay_bounds(
                monitor_position,
                monitor_size,
                1.5,
                OVERLAY_WIDTH,
                OVERLAY_HEIGHT,
                crate::settings::OverlayPosition::Top,
            ),
            (3648, 6, 384, 69)
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_overlay_bounds_support_negative_monitor_origins() {
        assert_eq!(
            windows_overlay_bounds(
                PhysicalPosition::new(-2560, -200),
                PhysicalSize::new(2560, 1440),
                1.25,
                OVERLAY_STREAM_WIDTH,
                OVERLAY_STREAM_HEIGHT,
                crate::settings::OverlayPosition::Bottom,
            ),
            (-1530, 1040, 500, 150)
        );
    }
}
