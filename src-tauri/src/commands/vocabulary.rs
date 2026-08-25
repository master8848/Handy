use crate::settings::{get_settings, write_settings, CustomWordDataset};
use tauri::AppHandle;

const MAX_DATASET_NAME_LEN: usize = 60;

/// Replaces the whole dataset list. The frontend manages individual datasets
/// (toggle, add/remove words, rename, delete) and saves the resulting list in
/// one shot, mirroring the flat `custom_words` update flow.
#[tauri::command]
#[specta::specta]
pub fn update_custom_word_datasets(
    app: AppHandle,
    datasets: Vec<CustomWordDataset>,
) -> Result<(), String> {
    let mut datasets = datasets;
    for dataset in &mut datasets {
        dataset.name = dataset
            .name
            .trim()
            .chars()
            .take(MAX_DATASET_NAME_LEN)
            .collect();
        dataset.words = crate::vocab::parse_terms(&dataset.words.join("\n"));
    }

    let mut settings = get_settings(&app);
    settings.custom_word_datasets = datasets;
    write_settings(&app, settings);
    Ok(())
}

/// Imports a text file (one term per line) as a new user dataset. Reading
/// happens in Rust so arbitrary user-picked paths work regardless of the
/// plugin-fs scope.
#[tauri::command]
#[specta::specta]
pub fn import_custom_word_dataset(
    app: AppHandle,
    path: String,
    name: Option<String>,
) -> Result<CustomWordDataset, String> {
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {e}"))?;
    let words = crate::vocab::parse_terms(&content);
    if words.is_empty() {
        return Err("The file contains no words.".to_string());
    }

    let fallback_name = std::path::Path::new(&path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.replace(['_', '-'], " "))
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or_else(|| "Imported Words".to_string());

    let name = name
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or(fallback_name);
    let name: String = name.chars().take(MAX_DATASET_NAME_LEN).collect();

    let id = format!(
        "user-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    );

    let dataset = CustomWordDataset {
        id,
        name,
        words,
        builtin: false,
        enabled: true,
    };

    let mut settings = get_settings(&app);
    settings.custom_word_datasets.push(dataset.clone());
    write_settings(&app, settings);
    Ok(dataset)
}

/// Exports a dataset (user or built-in) to a text file, one term per line.
#[tauri::command]
#[specta::specta]
pub fn export_custom_word_dataset(
    app: AppHandle,
    dataset_id: String,
    path: String,
) -> Result<(), String> {
    let settings = get_settings(&app);
    let dataset = settings
        .custom_word_datasets
        .iter()
        .find(|dataset| dataset.id == dataset_id)
        .ok_or_else(|| "Dataset not found.".to_string())?;

    let mut content = format!("# {} ({})\n", dataset.name, dataset.words.len());
    for word in &dataset.words {
        content.push_str(word);
        content.push('\n');
    }
    std::fs::write(&path, content).map_err(|e| format!("Failed to write file: {e}"))
}
