use crate::spellcheck::{SpellChecker, SpellingIssue};
use serde::Serialize;
use specta::Type;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

/// Current state of the offline spell checker, for the frontend to reflect.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub struct HarperStatus {
    /// Whether the Harper lint group has been built and is ready for checks.
    pub initialized: bool,
    /// Whether spell checking is globally enabled in settings.
    pub enabled: bool,
}

/// Check `text` for spelling and grammar issues using the offline Harper engine.
///
/// Heavy work (dictionary-backed linting) runs on a blocking worker so the UI
/// thread never stalls; the frontend is expected to debounce calls (300 ms).
#[tauri::command]
#[specta::specta]
pub async fn check_spelling(
    spell_checker: State<'_, Arc<SpellChecker>>,
    text: String,
) -> Result<Vec<SpellingIssue>, String> {
    let checker = spell_checker.inner().clone();
    tokio::task::spawn_blocking(move || checker.check(&text))
        .await
        .map_err(|e| format!("spell check task panicked: {e}"))?
        .map_err(|e| e.to_string())
}

/// Report whether the spell checker is initialized and enabled.
#[tauri::command]
#[specta::specta]
pub fn harper_status(
    app: AppHandle,
    spell_checker: State<'_, Arc<SpellChecker>>,
) -> Result<HarperStatus, String> {
    let settings = crate::settings::get_settings(&app);
    Ok(HarperStatus {
        initialized: spell_checker.is_initialized(),
        enabled: settings.spell_check_enabled,
    })
}

/// Persist the global spell-check enable/disable toggle. Enabling also kicks
/// off eager Harper initialization off the UI thread.
#[tauri::command]
#[specta::specta]
pub fn change_spell_check_enabled_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = crate::settings::get_settings(&app);
    settings.spell_check_enabled = enabled;
    crate::settings::write_settings(&app, settings);

    if enabled {
        let checker = Arc::clone(&app.state::<Arc<SpellChecker>>());
        std::thread::spawn(move || checker.ensure_initialized());
    }

    Ok(())
}
