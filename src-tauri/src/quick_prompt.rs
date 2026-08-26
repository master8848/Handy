use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const LABEL: &str = "quick_prompt";
const TITLE: &str = "Quick Prompt";

/// Logical size for the spotlight-like prompt box.
/// Keep in sync with the frontend container (QuickPromptBox).
pub const QUICK_PROMPT_WIDTH: f64 = 640.0;
pub const QUICK_PROMPT_HEIGHT: f64 = 280.0;

/// Returns true if the quick prompt window is currently visible.
fn is_visible(app: &AppHandle) -> bool {
    app.get_webview_window(LABEL)
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false)
}

fn ensure_window(app: &AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(win) = app.get_webview_window(LABEL) {
        return Ok(win);
    }

    let mut builder = WebviewWindowBuilder::new(
        app,
        LABEL,
        WebviewUrl::App("/?view=quick-prompt".into()),
    )
    .title(TITLE)
    .inner_size(QUICK_PROMPT_WIDTH, QUICK_PROMPT_HEIGHT)
    .min_inner_size(quick_prompt_min_width(), quick_prompt_min_height())
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .center()
    .focusable(true)
    .closable(false);

    if let Some(data_dir) = crate::portable::data_dir() {
        builder = builder.data_directory(data_dir.join("webview"));
    }

    builder.build().map_err(|e| e.to_string())
}

fn quick_prompt_min_width() -> f64 { 520.0 }
fn quick_prompt_min_height() -> f64 { 200.0 }

/// Create the hidden quick prompt window at startup (best-effort).
pub fn create_quick_prompt_window(app: &AppHandle) {
    // Don't fail startup if window creation fails — it can be lazily created on toggle.
    let _ = ensure_window(app);
}

/// Toggle: if visible hide, otherwise show and focus.
/// Centers on the monitor with the cursor, similar to Spotlight behavior.
pub fn toggle_quick_prompt(app: &AppHandle) {
    if is_visible(app) {
        hide_quick_prompt(app);
        return;
    }
    show_quick_prompt(app);
}

pub fn show_quick_prompt(app: &AppHandle) {
    let win = match ensure_window(app) {
        Ok(w) => w,
        Err(e) => {
            log::error!("Failed to create quick prompt window: {}", e);
            return;
        }
    };

    // Re-center on the monitor with cursor for multi-monitor setups
    if let Some((x, y)) = calculate_centered_position(app, QUICK_PROMPT_WIDTH, QUICK_PROMPT_HEIGHT) {
        let _ = win.set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
    }

    let _ = win.show();
    let _ = win.unminimize();
    let _ = win.set_focus();
    // Bring to front on Windows
    #[cfg(target_os = "windows")]
    {
        let _ = win.set_always_on_top(true);
    }
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    }
    let _ = win.emit("quick-prompt:opened", ());
}

pub fn hide_quick_prompt(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(LABEL) {
        let _ = win.hide();
        #[cfg(target_os = "macos")]
        {
            // Return to accessory so previous app regains focus after we hide.
            // The paste logic does a short sleep to let the focus switch settle.
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
        }
    }
}

fn calculate_centered_position(app: &AppHandle, width: f64, height: f64) -> Option<(f64, f64)> {
    let monitor = crate::overlay::get_monitor_with_cursor(app)?;
    let scale = monitor.scale_factor();
    let monitor_x = monitor.position().x as f64 / scale;
    let monitor_y = monitor.position().y as f64 / scale;
    let monitor_width = monitor.size().width as f64 / scale;
    let monitor_height = monitor.size().height as f64 / scale;

    let x = monitor_x + (monitor_width - width) / 2.0;
    // Slightly above center vertically (Spotlight is ~30% from top)
    let y = monitor_y + (monitor_height - height) / 2.0 - 80.0;
    Some((x, y.max(monitor_y + 20.0)))
}

/// Paste text into the previously focused app.
///
/// Hides the quick prompt window, optionally saves to history, then pastes via
/// the shared clipboard pipeline. Hiding first lets the OS restore focus to
/// the previous app so the keystroke lands in the right place.
///
/// Called from the frontend on Cmd/Ctrl+Enter.
#[tauri::command]
#[specta::specta]
pub fn quick_prompt_paste(
    app: AppHandle,
    prompt_history: tauri::State<'_, std::sync::Arc<crate::managers::prompt_history::PromptHistoryManager>>,
    text: String,
) -> Result<crate::managers::prompt_history::PromptHistoryEntry, String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("empty prompt".into());
    }

    // Hide window on main thread before paste so previous app regains focus.
    let app_for_hide = app.clone();
    let _ = app.run_on_main_thread(move || {
        hide_quick_prompt(&app_for_hide);
    });

    // Small delay to let the window hide and focus switch to previous app.
    // Without this, the paste keystroke can be swallowed by our own window,
    // especially on macOS where hide is async.
    std::thread::sleep(std::time::Duration::from_millis(120));

    crate::clipboard::paste(text.clone(), app.clone())?;
    prompt_history.save_entry(text).map_err(|e| e.to_string())
}

/// Just hide without pasting (Esc).
#[tauri::command]
#[specta::specta]
pub fn hide_quick_prompt_command(app: AppHandle) -> Result<(), String> {
    let app_clone = app.clone();
    let _ = app.run_on_main_thread(move || {
        hide_quick_prompt(&app_clone);
    });
    Ok(())
}

/// Show command (for manual triggering / debugging).
#[tauri::command]
#[specta::specta]
pub fn show_quick_prompt_command(app: AppHandle) -> Result<(), String> {
    show_quick_prompt(&app);
    Ok(())
}
