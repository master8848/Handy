use serde::{Deserialize, Serialize};

/// Events emitted by handy_core, decoupled from any UI framework.
/// Mirrors the event names Handy emits through Tauri.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CoreEvent {
    ModelsUpdated,
    ModelDeleted {
        model_id: String,
    },
    ModelDownloadProgress {
        model_id: String,
        downloaded: u64,
        total: u64,
        percentage: f64,
    },
    ModelDownloadComplete {
        model_id: String,
    },
    ModelDownloadCancelled {
        model_id: String,
    },
    ModelExtractionStarted {
        model_id: String,
    },
    ModelExtractionComplete {
        model_id: String,
    },
    ModelExtractionFailed {
        model_id: String,
        error: Option<String>,
    },
    ModelVerificationStarted {
        model_id: String,
    },
    ModelVerificationComplete {
        model_id: String,
    },
    ModelVerificationFailed {
        model_id: String,
        error: Option<String>,
    },
    /// Mirrors Handy's "model-state-changed" event.
    ModelStateChanged {
        event_type: String,
        model_id: Option<String>,
        model_name: Option<String>,
        error: Option<String>,
    },
    /// Streaming transcription text (committed + tentative).
    StreamText {
        committed: String,
        tentative: String,
    },
    /// Streaming phase changes ("listening", "processing", ...).
    StreamPhase {
        phase: String,
    },
    /// Microphone level buckets for visualization (16 values, 0.0-1.0).
    AudioLevels(Vec<f32>),
}

/// Consumer for core events. The UI wires this to its own channel.
pub trait EventSink: Send + Sync {
    fn on_event(&self, event: CoreEvent);
}

impl<F> EventSink for F
where
    F: Fn(CoreEvent) + Send + Sync,
{
    fn on_event(&self, event: CoreEvent) {
        self(event)
    }
}
