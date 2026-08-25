use crate::clipboard;
use crate::managers::prompt_history::{PromptHistoryEntry, PromptHistoryManager};
use std::sync::Arc;
use tauri::{AppHandle, State};

/// Paste `text` into the active application (same clipboard-paste pipeline as
/// transcription output) and record it in prompt history so the user can
/// review what was pasted and when. Nothing is recorded when the paste fails.
#[tauri::command]
#[specta::specta]
pub fn paste_prompt(
    app: AppHandle,
    prompt_history: State<'_, Arc<PromptHistoryManager>>,
    text: String,
) -> Result<PromptHistoryEntry, String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("empty prompt".into());
    }

    clipboard::paste(text.clone(), app)?;
    prompt_history.save_entry(text).map_err(|e| e.to_string())
}

/// Record a prompt written in the Prompt Studio. The frontend auto-saves with
/// a debounce, so written prompts show up in prompt history too — not just
/// pasted ones. Deduplication keeps re-edits from piling up copies.
#[tauri::command]
#[specta::specta]
pub fn save_prompt_history_entry(
    prompt_history: State<'_, Arc<PromptHistoryManager>>,
    text: String,
) -> Result<PromptHistoryEntry, String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("empty prompt".into());
    }
    prompt_history.save_entry(text).map_err(|e| e.to_string())
}

/// List pasted prompts, newest first.
#[tauri::command]
#[specta::specta]
pub fn list_prompt_history(
    prompt_history: State<'_, Arc<PromptHistoryManager>>,
    limit: Option<usize>,
) -> Result<Vec<PromptHistoryEntry>, String> {
    prompt_history
        .list_entries(limit)
        .map_err(|e| e.to_string())
}

/// Delete a single prompt history entry.
#[tauri::command]
#[specta::specta]
pub fn delete_prompt_history_entry(
    prompt_history: State<'_, Arc<PromptHistoryManager>>,
    id: i64,
) -> Result<(), String> {
    prompt_history.delete_entry(id).map_err(|e| e.to_string())
}

/// Clear the entire prompt history.
#[tauri::command]
#[specta::specta]
pub fn clear_prompt_history(
    prompt_history: State<'_, Arc<PromptHistoryManager>>,
) -> Result<(), String> {
    prompt_history.clear().map_err(|e| e.to_string())
}
