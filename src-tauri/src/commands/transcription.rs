use crate::managers::transcription::TranscriptionManager;
use crate::settings::{get_settings, write_settings, ModelUnloadTimeout};
use serde::Serialize;
use specta::Type;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Serialize, Type)]
pub struct ModelLoadStatus {
    is_loaded: bool,
    current_model: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub fn set_model_unload_timeout(app: AppHandle, timeout: ModelUnloadTimeout) {
    let mut settings = get_settings(&app);
    settings.model_unload_timeout = timeout;
    write_settings(&app, settings);
}

#[tauri::command]
#[specta::specta]
pub fn get_model_load_status(
    transcription_manager: State<TranscriptionManager>,
) -> Result<ModelLoadStatus, String> {
    Ok(ModelLoadStatus {
        is_loaded: transcription_manager.is_model_loaded(),
        current_model: transcription_manager.get_current_model(),
    })
}

#[tauri::command]
#[specta::specta]
pub fn unload_model_manually(
    transcription_manager: State<TranscriptionManager>,
) -> Result<(), String> {
    transcription_manager
        .unload_model()
        .map_err(|e| format!("Failed to unload model: {}", e))
}

#[tauri::command]
#[specta::specta]
pub fn unload_model_by_id(
    transcription_manager: State<TranscriptionManager>,
    model_id: String,
) -> Result<(), String> {
    transcription_manager
        .unload_model_by_id(&model_id)
        .map_err(|e| format!("Failed to unload model: {}", e))
}

/// One-shot transcription of a user-picked audio file (mp3, wav, m4a, ogg,
/// flac, …). Decodes to 16 kHz mono PCM, loads the requested (or selected)
/// model, transcribes, and returns the finished text with text replacements
/// applied. Progress is emitted on the `file-transcription-status` event.
#[tauri::command]
#[specta::specta]
pub async fn transcribe_audio_file(
    app: AppHandle,
    path: String,
    model_id: Option<String>,
) -> Result<String, String> {
    use crate::actions::process_transcription_output;

    let emit = |phase: &str| {
        let _ = app.emit("file-transcription-status", phase.to_string());
    };

    emit("decoding");
    let samples = tauri::async_runtime::spawn_blocking(move || {
        crate::audio_toolkit::decode_audio_file_to_samples(&path)
    })
    .await
    .map_err(|e| format!("Audio decoding task panicked: {}", e))??;

    if samples.is_empty() {
        return Err("No audio could be decoded from this file".to_string());
    }

    emit("loading_model");
    let tm = Arc::clone(&app.state::<Arc<TranscriptionManager>>());
    let target_model = model_id
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| crate::settings::get_settings(&app).selected_model);
    if target_model.is_empty() {
        return Err(
            "No model selected. Pick a model in Settings → Models, then try again.".to_string(),
        );
    }

    // The Windows OS speech backend transcribes from a file, so the decoded
    // samples must be materialized to a temporary WAV when os-speech is the
    // target (macOS accepts raw PCM; the file is still cleaned up).
    let os_wav_path = if target_model == crate::managers::model::OS_SPEECH_MODEL_ID {
        Some(
            crate::audio_toolkit::save_temp_wav_file(&samples)
                .map_err(|e| format!("Failed to write temporary WAV: {e}"))?,
        )
    } else {
        None
    };

    let transcribe_result = tauri::async_runtime::spawn_blocking(move || {
        // Load the model synchronously (short-circuits when already loaded).
        let result = (|| {
            tm.load_model_with_device(&target_model, None)
                .map_err(|e| format!("Failed to load model '{}': {}", target_model, e))?;
            tm.transcribe_with_wav_path(samples, os_wav_path.as_deref())
                .map_err(|e| e.to_string())
        })();
        if let Some(path) = os_wav_path {
            let _ = std::fs::remove_file(path);
        }
        result
    })
    .await
    .map_err(|e| format!("Transcription task panicked: {}", e))?;

    let transcription = transcribe_result?;
    if transcription.trim().is_empty() {
        return Err("No speech was detected in this file".to_string());
    }

    emit("finalizing");
    let processed = process_transcription_output(&app, &transcription, false).await;

    Ok(processed.final_text)
}
