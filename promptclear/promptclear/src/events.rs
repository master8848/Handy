// PromptClear — offline voice notes. MIT.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Events flowing from the core and worker threads into the UI thread.
pub enum UiEvent {
    Core(handy_core::CoreEvent),
    TranscriptionResult { text: String },
    TranscriptionError { error: String },
}

/// Global-hotkey messages from the rdev listener thread.
pub enum Hotkey {
    ToggleRecording,
}

/// A shared channel between the core (via an `EventSink` adapter) and the app.
pub struct EventBus {
    pub tx: std::sync::mpsc::Sender<UiEvent>,
    pub rx: std::sync::mpsc::Receiver<UiEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self { tx, rx }
    }

    /// An `EventSink` adapter that forwards core events onto the bus and wakes
    /// the UI thread so events are processed while the app is unfocused.
    /// Mic-level events stream continuously while the mic stream is open
    /// (AlwaysOn / lazy-close window) — those don't wake the UI unless a
    /// recording is actually active.
    pub fn sink(
        &self,
        ctx: egui::Context,
        recording: Arc<AtomicBool>,
    ) -> impl handy_core::EventSink {
        let tx = self.tx.clone();
        move |event: handy_core::CoreEvent| {
            let is_levels = matches!(&event, handy_core::CoreEvent::AudioLevels(_));
            let _ = tx.send(UiEvent::Core(event));
            if is_levels && !recording.load(Ordering::Relaxed) {
                return;
            }
            ctx.request_repaint();
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
