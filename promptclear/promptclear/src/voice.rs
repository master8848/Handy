// PromptClear — offline voice notes. MIT.

use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use handy_core::transcription::StreamWorkKind;
use handy_core::{AudioRecordingManager, ModelManager, TranscriptionManager, VadPolicy};

use crate::events::UiEvent;

/// Recording + transcription pipeline on top of handy_core.
pub struct VoicePipeline {
    pub audio_mgr: Arc<AudioRecordingManager>,
    pub tm: Arc<TranscriptionManager>,
    pub model_mgr: Arc<ModelManager>,
    tx: Sender<UiEvent>,
}

impl VoicePipeline {
    pub fn new(
        audio_mgr: Arc<AudioRecordingManager>,
        tm: Arc<TranscriptionManager>,
        model_mgr: Arc<ModelManager>,
        tx: Sender<UiEvent>,
    ) -> Self {
        Self {
            audio_mgr,
            tm,
            model_mgr,
            tx,
        }
    }

    pub fn toggle(&self) {
        if self.audio_mgr.is_recording() {
            self.stop_and_transcribe();
        } else {
            self.start_recording();
        }
    }

    /// Start a recording session. Returns `true` when a live streaming session
    /// was started (committed text arrives via `CoreEvent::StreamText`), `false`
    /// for the offline record→transcribe→append path.
    pub fn start_recording(&self) -> bool {
        if !self.tm.is_model_loaded() {
            if self.pick_model_id().is_none() {
                let _ = self.tx.send(UiEvent::TranscriptionError {
                    error: "No model available — download one from the top bar or Settings."
                        .to_string(),
                });
                return false;
            }
            let tm = self.tm.clone();
            std::thread::spawn(move || {
                tm.initiate_model_load();
            });
        }

        let streaming = self.streaming_enabled()
            && self.tm.is_model_loaded()
            && self.model_supports_streaming();
        if streaming {
            // The recorder's audio callback feeds the stream router directly, so
            // opening the route (`start_stream`) is what makes live frames flow.
            // Must run before recording starts or frames are dropped.
            self.tm.start_stream();
        }

        let vad_policy = if streaming {
            VadPolicy::Streaming
        } else {
            VadPolicy::Offline
        };
        match self.audio_mgr.try_start_recording("mic", vad_policy) {
            Ok(()) => streaming,
            Err(error) => {
                if streaming {
                    self.tm.cancel_stream();
                }
                let _ = self.tx.send(UiEvent::TranscriptionError { error });
                false
            }
        }
    }

    fn streaming_enabled(&self) -> bool {
        self.audio_mgr
            .settings
            .lock()
            .map(|settings| settings.streaming_enabled)
            .unwrap_or(false)
    }

    fn model_supports_streaming(&self) -> bool {
        let id = self.tm.get_current_model().or_else(|| self.pick_model_id());
        let Some(id) = id else {
            return false;
        };
        self.model_mgr
            .get_available_models()
            .iter()
            .find(|model| model.id == id)
            .map(|model| model.supports_streaming)
            .unwrap_or(false)
    }

    fn pick_model_id(&self) -> Option<String> {
        let selected = self
            .audio_mgr
            .settings
            .lock()
            .ok()
            .map(|settings| settings.selected_model.clone())
            .unwrap_or_default();
        let models = self.model_mgr.get_available_models();

        if !selected.is_empty() && models.iter().any(|model| model.id == selected) {
            return Some(selected);
        }
        models
            .iter()
            .find(|model| model.is_downloaded)
            .map(|model| model.id.clone())
            .or_else(|| models.first().map(|model| model.id.clone()))
    }

    pub fn stop_and_transcribe(&self) {
        if self.tm.is_streaming() {
            self.tm.emit_stream_working(StreamWorkKind::Transcribing);
        }

        let cancel_generation = self.audio_mgr.cancel_generation();
        match self.audio_mgr.stop_recording("mic", cancel_generation) {
            Some(samples) => {
                if self.audio_mgr.was_cancelled_since(cancel_generation) {
                    self.tm.cancel_stream();
                    return;
                }
                if samples.is_empty() {
                    self.tm.cancel_stream();
                    let _ = self.tx.send(UiEvent::TranscriptionError {
                        error: "No audio captured".to_string(),
                    });
                    return;
                }
                let tm = self.tm.clone();
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    // A finalized stream with usable text wins; an empty stream
                    // result falls back to batch transcription of the same audio.
                    // A finalize error is surfaced — the worker may still hold the
                    // engine, so a batch fallback could contend with it.
                    let result = match tm.finalize_stream() {
                        Ok(Some(text)) if !text.trim().is_empty() => Ok(text),
                        Ok(_) => tm.transcribe(samples),
                        Err(error) => Err(error),
                    };
                    let _ = match result {
                        Ok(text) => tx.send(UiEvent::TranscriptionResult { text }),
                        Err(error) => tx.send(UiEvent::TranscriptionError {
                            error: error.to_string(),
                        }),
                    };
                });
            }
            None => {
                self.tm.cancel_stream();
                let _ = self.tx.send(UiEvent::TranscriptionError {
                    error: "No audio captured".to_string(),
                });
            }
        }
    }

    pub fn cancel(&self) {
        self.audio_mgr.cancel_recording();
        self.tm.cancel_stream();
    }

    pub fn start_download(&self, model_id: &str) {
        let model_mgr = self.model_mgr.clone();
        let model_id = model_id.to_string();
        std::thread::spawn(move || {
            if let Err(error) = model_mgr.download_model_sync(&model_id) {
                log::error!("model download failed: {error}");
            }
        });
    }

    pub fn register_custom_model(&self, path: PathBuf) {
        let model_mgr = self.model_mgr.clone();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            if let Err(error) = model_mgr.register_custom_model(path) {
                let _ = tx.send(UiEvent::TranscriptionError {
                    error: error.to_string(),
                });
            }
        });
    }

    pub fn delete_model(&self, model_id: &str) {
        if let Err(error) = self.model_mgr.delete_model(model_id) {
            let _ = self.tx.send(UiEvent::TranscriptionError {
                error: error.to_string(),
            });
        }
    }
}
