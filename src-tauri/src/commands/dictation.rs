use crate::audio_toolkit::VadPolicy;
use crate::managers::audio::AudioRecordingManager;
use crate::managers::transcription::TranscriptionManager;
use crate::settings::get_settings;
use std::sync::Arc;
use tauri::{AppHandle, State};

const DICTATION_BINDING_ID: &str = "dictation";

/// Start a dictation recording. Mirrors the transcribe shortcut start path
/// without overlay, tray icon, or feedback sounds.
#[tauri::command]
#[specta::specta]
pub fn start_dictation(
    app: AppHandle,
    audio_manager: State<'_, Arc<AudioRecordingManager>>,
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
) -> Result<(), String> {
    if audio_manager.is_recording() {
        return Err("already recording".into());
    }

    let settings = get_settings(&app);
    if settings.selected_model.is_empty() {
        return Err("no model selected".into());
    }

    transcription_manager.initiate_model_load();

    let vad_policy = if !settings.vad_enabled {
        VadPolicy::Disabled
    } else {
        VadPolicy::Offline
    };
    audio_manager
        .try_start_recording(DICTATION_BINDING_ID, vad_policy)
        .map_err(|e| e.to_string())
}

/// Stop the current dictation recording and transcribe it, returning the text.
#[tauri::command]
#[specta::specta]
pub async fn stop_dictation(
    _app: AppHandle,
    audio_manager: State<'_, Arc<AudioRecordingManager>>,
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
) -> Result<String, String> {
    let cancel_generation = audio_manager.cancel_generation();

    let samples = match audio_manager.stop_recording(DICTATION_BINDING_ID, cancel_generation) {
        Some(samples) => samples,
        None => {
            if audio_manager.was_cancelled_since(cancel_generation) {
                transcription_manager.cancel_stream();
                return Err("cancelled".into());
            }
            transcription_manager.cancel_stream();
            return Err("empty".into());
        }
    };

    if samples.is_empty() {
        transcription_manager.cancel_stream();
        return Err("empty".into());
    }

    let tm = Arc::clone(&transcription_manager);
    let os_wav_path = if transcription_manager.get_current_model().as_deref()
        == Some(crate::managers::model::OS_SPEECH_MODEL_ID)
    {
        // The Windows OS speech backend transcribes from a file; macOS accepts
        // raw PCM (the temp file is still cleaned up either way).
        Some(
            crate::audio_toolkit::save_temp_wav_file(&samples)
                .map_err(|e| format!("Failed to write temporary WAV: {e}"))?,
        )
    } else {
        None
    };
    let transcription = tauri::async_runtime::spawn_blocking(move || {
        let result = tm.transcribe_with_wav_path(samples, os_wav_path.as_deref());
        if let Some(path) = os_wav_path {
            let _ = std::fs::remove_file(path);
        }
        result.map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Transcription task panicked: {}", e))??;

    if transcription.trim().is_empty() {
        Err("no speech".into())
    } else {
        Ok(transcription)
    }
}

/// Cancel the current dictation recording and tear down any streaming worker.
#[tauri::command]
#[specta::specta]
pub fn cancel_dictation(
    _app: AppHandle,
    audio_manager: State<'_, Arc<AudioRecordingManager>>,
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
) -> Result<(), String> {
    audio_manager.cancel_recording();
    transcription_manager.cancel_stream();
    Ok(())
}
