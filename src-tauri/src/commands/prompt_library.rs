use crate::managers::prompt_library::{
    render_prompt_content, Folder, Prompt, PromptFilter, PromptLibraryManager, PromptVersion, Tag,
};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
#[specta::specta]
pub fn list_prompts(
    prompt_library: State<'_, Arc<PromptLibraryManager>>,
    filter: Option<PromptFilter>,
) -> Result<Vec<Prompt>, String> {
    prompt_library
        .list_prompts(filter.unwrap_or_default())
        .map_err(|e| e.to_string())
}

/// FTS5 search — `MATCH` + `bm25(prompt_fts, 10.0, 5.0, 1.0)` + `pinned` tie-breaker,
/// `LIMIT 50`. See `managers::prompt_library::PromptLibraryManager::search_prompts`
/// for ranking/debounce/trigger docs. Wrapped in `spawn_blocking` like
/// `commands::spellcheck::check_spelling` so SQLite I/O never blocks the async
/// runtime. On `MATCH` syntax error the manager falls back to `LIKE '%q%'`.
#[tauri::command]
#[specta::specta]
pub async fn search_prompts(
    prompt_library: State<'_, Arc<PromptLibraryManager>>,
    query: String,
    folder_id: Option<i64>,
    limit: Option<i64>,
) -> Result<Vec<Prompt>, String> {
    let manager = prompt_library.inner().clone();
    tokio::task::spawn_blocking(move || {
        manager
            .search_prompts(&query, folder_id, limit)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("search task panicked: {e}"))?
}

#[tauri::command]
#[specta::specta]
pub fn get_prompt(
    prompt_library: State<'_, Arc<PromptLibraryManager>>,
    id: i64,
) -> Result<Prompt, String> {
    prompt_library.get_prompt(id).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn create_prompt(
    prompt_library: State<'_, Arc<PromptLibraryManager>>,
    title: String,
    content: String,
    folder_id: Option<i64>,
    tags: Option<Vec<String>>,
) -> Result<Prompt, String> {
    prompt_library
        .create_prompt(title, content, folder_id, tags.unwrap_or_default())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn update_prompt(
    prompt_library: State<'_, Arc<PromptLibraryManager>>,
    id: i64,
    title: String,
    content: String,
    folder_id: Option<i64>,
    tags: Option<Vec<String>>,
) -> Result<Prompt, String> {
    prompt_library
        .update_prompt(id, title, content, folder_id, tags.unwrap_or_default())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn delete_prompt(
    prompt_library: State<'_, Arc<PromptLibraryManager>>,
    id: i64,
) -> Result<(), String> {
    prompt_library.delete_prompt(id).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn duplicate_prompt(
    prompt_library: State<'_, Arc<PromptLibraryManager>>,
    id: i64,
) -> Result<Prompt, String> {
    prompt_library
        .duplicate_prompt(id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn toggle_prompt_pin(
    prompt_library: State<'_, Arc<PromptLibraryManager>>,
    id: i64,
) -> Result<Prompt, String> {
    prompt_library.toggle_pin(id).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn increment_prompt_usage(
    prompt_library: State<'_, Arc<PromptLibraryManager>>,
    id: i64,
) -> Result<(), String> {
    prompt_library
        .increment_usage(id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn insert_prompt(
    app: AppHandle,
    prompt_library: State<'_, Arc<PromptLibraryManager>>,
    id: i64,
    variables: Option<HashMap<String, String>>,
) -> Result<(), String> {
    let prompt = prompt_library.get_prompt(id).map_err(|e| e.to_string())?;
    let rendered = if let Some(vars) = variables {
        render_prompt_content(&prompt.content, &vars)
    } else {
        prompt.content.clone()
    };
    if rendered.contains("{{") {
        return Err("prompt still contains unfilled variables".to_string());
    }
    crate::clipboard::paste(rendered, app.clone()).map_err(|e| e.to_string())?;
    let _ = prompt_library.increment_usage(id);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn list_folders(
    prompt_library: State<'_, Arc<PromptLibraryManager>>,
) -> Result<Vec<Folder>, String> {
    prompt_library.list_folders().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn create_folder(
    prompt_library: State<'_, Arc<PromptLibraryManager>>,
    name: String,
    color: Option<String>,
) -> Result<Folder, String> {
    prompt_library
        .create_folder(name, color)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn update_folder(
    prompt_library: State<'_, Arc<PromptLibraryManager>>,
    id: i64,
    name: String,
    color: Option<String>,
) -> Result<Folder, String> {
    prompt_library
        .update_folder(id, name, color)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn delete_folder(
    prompt_library: State<'_, Arc<PromptLibraryManager>>,
    id: i64,
) -> Result<(), String> {
    prompt_library.delete_folder(id).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn list_tags(prompt_library: State<'_, Arc<PromptLibraryManager>>) -> Result<Vec<Tag>, String> {
    prompt_library.list_tags().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn list_prompt_versions(
    prompt_library: State<'_, Arc<PromptLibraryManager>>,
    prompt_id: i64,
) -> Result<Vec<PromptVersion>, String> {
    prompt_library
        .list_versions(prompt_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn restore_prompt_version(
    prompt_library: State<'_, Arc<PromptLibraryManager>>,
    prompt_id: i64,
    version: i64,
) -> Result<Prompt, String> {
    prompt_library
        .restore_version(prompt_id, version)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn export_prompts(
    prompt_library: State<'_, Arc<PromptLibraryManager>>,
) -> Result<String, String> {
    prompt_library.export_json().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn import_prompts(
    prompt_library: State<'_, Arc<PromptLibraryManager>>,
    json: String,
) -> Result<usize, String> {
    prompt_library.import_json(json).map_err(|e| e.to_string())
}
