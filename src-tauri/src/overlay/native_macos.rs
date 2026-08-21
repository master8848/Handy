use std::ffi::CString;
use std::os::raw::{c_char, c_double};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::AppHandle;

use super::{calculate_overlay_position, overlay_dimensions};

static LAST_MIC_LEVEL_EMIT: AtomicU64 = AtomicU64::new(0);
const EMIT_THROTTLE_MS: u64 = 16; // 60fps

// Track last shown state so update_overlay_position can re-assert bounds without
// knowing which `show_*` was last called.
static LAST_STATE: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

extern "C" {
    fn overlay_show(state: *const c_char, x: c_double, y: c_double, w: c_double, h: c_double);
    fn overlay_hide();
    fn overlay_set_levels(levels: *const f32, count: usize);
    fn overlay_set_stream_text(
        committed: *const c_char,
        tentative: *const c_char,
        phase: *const c_char,
        work_kind: *const c_char,
    );
}

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
    let (w, h) = overlay_dimensions(state);
    // Use calculate_overlay_position to keep per-monitor DPI logic unified with
    // the WebView path. NSPanel.setFrameOrigin takes logical points.
    let pos = calculate_overlay_position(app_handle, w, h);
    // Hop to main thread: although overlay_panel.swift dispatches to main,
    // setFrameOrigin geometry queries (NSScreen) are main-thread-only on macOS.
    let handle = app_handle.clone();
    let state_owned = state.to_string();
    let _ = app_handle.run_on_main_thread(move || {
        let (x, y) = pos.unwrap_or((100.0, 100.0));
        let c_state =
            CString::new(state_owned).unwrap_or_else(|_| CString::new("recording").unwrap());
        unsafe {
            overlay_show(c_state.as_ptr(), x, y, w, h);
        }
        // Ensure panel is frontmost across Spaces (NSPanel Level::Status + canJoinAllSpaces already set)
        let _ = &handle;
    });
}

pub fn create_recording_overlay(_app_handle: &AppHandle) {
    // Lazy — panel is created on first show. Create here is a no-op besides
    // ensuring the Swift static is linked. Keep hidden.
    log::debug!("native macOS overlay: create (lazy)");
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

pub fn hide_recording_overlay(_app_handle: &AppHandle) {
    let app = _app_handle.clone();
    let _ = app.run_on_main_thread(move || unsafe {
        overlay_hide();
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

    unsafe {
        overlay_set_levels(levels.as_ptr(), levels.len());
    }
}

// Optional: stream text bridge for future use (kept for parity; current
// streaming text still goes via Tauri events, native additionally via FFI).
#[allow(dead_code)]
pub fn set_stream_text(committed: &str, tentative: &str, phase: &str, work_kind: &str) {
    let c_committed = CString::new(committed).unwrap_or_default();
    let c_tentative = CString::new(tentative).unwrap_or_default();
    let c_phase = CString::new(phase).unwrap_or_default();
    let c_work = CString::new(work_kind).unwrap_or_default();
    unsafe {
        overlay_set_stream_text(
            c_committed.as_ptr(),
            c_tentative.as_ptr(),
            c_phase.as_ptr(),
            c_work.as_ptr(),
        );
    }
}
