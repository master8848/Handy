// PromptClear — offline voice notes. MIT.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::App;
use handy_core::settings::{get_settings, write_settings, AppSettings, SettingsStore};
use handy_core::{CoreEvent, ModelInfo};

use crate::events::{EventBus, Hotkey, UiEvent};
use crate::ui;
use crate::voice::VoicePipeline;

pub(crate) enum AppStatus {
    Ready,
    ModelLoading,
    ModelDownloading { percentage: f64 },
    Recording,
    Transcribing,
    Error(String),
}

pub struct PromptClearApp {
    pub(crate) text: String,
    pub(crate) status: AppStatus,
    pub(crate) spell_latency_ms: Option<f64>,
    pub(crate) mic_levels: Vec<f32>,
    pub(crate) models: Vec<ModelInfo>,
    pub(crate) downloading: Option<(String, f64)>,
    pub(crate) settings_open: bool,
    /// Committed stream text already appended to `text` this session.
    pub(crate) stream_committed: String,
    /// Volatile tail of the live transcription, displayed under the editor.
    pub(crate) tentative: String,
    /// Streaming phase ("listening"/"working") for the status bar.
    pub(crate) stream_phase: Option<String>,
    /// True once the session's stream was finalized or cancelled; late
    /// `StreamText` events must not append after that.
    pub(crate) stream_finalized: bool,
    bus: EventBus,
    hotkey_rx: Receiver<Hotkey>,
    pub(crate) voice: VoicePipeline,
    pub(crate) store: Arc<dyn SettingsStore>,
    pub(crate) app_settings: AppSettings,
    pub(crate) last_recording_started: Option<Instant>,
    pub(crate) send_confirmation: Option<String>,
    /// The editor requests keyboard focus once, on launch.
    pub(crate) editor_focus_requested: bool,
    /// A stop was issued and the transcribe thread has not yet reported back;
    /// new recording starts are refused until it resolves.
    pub(crate) transcribe_pending: bool,
    /// An Esc happened while `transcribe_pending`; the late result is dropped.
    pub(crate) discard_pending: bool,
    /// Mirrors "a recording session is active" for the event sink (which must
    /// not wake the UI for mic-level events while idle) and gates level bars.
    pub(crate) recording_flag: Arc<AtomicBool>,
}

impl PromptClearApp {
    pub fn new(
        bus: EventBus,
        hotkey_rx: Receiver<Hotkey>,
        voice: VoicePipeline,
        store: Arc<dyn SettingsStore>,
        recording_flag: Arc<AtomicBool>,
    ) -> Self {
        let app_settings = get_settings(store.as_ref());
        let models = voice.model_mgr.get_available_models();
        Self {
            text: String::new(),
            status: AppStatus::Ready,
            spell_latency_ms: None,
            mic_levels: Vec::new(),
            models,
            downloading: None,
            settings_open: false,
            stream_committed: String::new(),
            tentative: String::new(),
            stream_phase: None,
            stream_finalized: false,
            bus,
            hotkey_rx,
            voice,
            store,
            app_settings,
            last_recording_started: None,
            send_confirmation: None,
            editor_focus_requested: false,
            transcribe_pending: false,
            discard_pending: false,
            recording_flag,
        }
    }

    pub(crate) fn toggle_recording(&mut self) {
        let was_recording = self.voice.audio_mgr.is_recording();
        if was_recording {
            self.voice.stop_and_transcribe();
            self.last_recording_started = None;
            self.stream_phase = None;
            self.transcribe_pending = true;
            self.recording_flag.store(false, Ordering::Relaxed);
        } else {
            if self.transcribe_pending {
                // The previous dictation is still being transcribed; a new
                // session would race the engine for the model.
                self.status = AppStatus::Transcribing;
                return;
            }
            self.stream_committed.clear();
            self.tentative.clear();
            self.stream_phase = None;
            self.stream_finalized = false;
            self.last_recording_started = Some(Instant::now());
            let streaming = self.voice.start_recording();
            self.recording_flag
                .store(self.voice.audio_mgr.is_recording(), Ordering::Relaxed);
            if streaming {
                self.stream_phase = Some("listening".to_string());
                if !self.text.is_empty() && !self.text.ends_with(char::is_whitespace) {
                    self.text.push('\n');
                }
            }
        }
    }

    pub(crate) fn send_text(&mut self, ctx: &egui::Context) {
        if self.text.trim().is_empty() {
            return;
        }
        ctx.copy_text(self.text.clone());
        self.text.clear();
        self.status = AppStatus::Ready;
        self.send_confirmation = Some("Copied to clipboard".to_string());
    }

    pub(crate) fn persist_settings(&mut self) {
        if let Err(error) = write_settings(self.store.as_ref(), &self.app_settings) {
            log::error!("failed to save settings: {error}");
        }
        if let Ok(mut settings) = self.voice.model_mgr.settings.lock() {
            *settings = self.app_settings.clone();
        }
        if let Ok(mut settings) = self.voice.audio_mgr.settings.lock() {
            *settings = self.app_settings.clone();
        }
    }

    /// Re-applies the shadcn theme after an appearance setting changed.
    pub(crate) fn apply_theme(&self, ctx: &egui::Context) {
        crate::theme::apply(
            ctx,
            self.app_settings.theme_mode,
            self.app_settings.accent_color,
            self.app_settings.corner_radius,
        );
    }

    fn process_events(&mut self) {
        let events: Vec<UiEvent> = self.bus.rx.try_iter().collect();
        for event in events {
            match event {
                UiEvent::Core(event) => self.handle_core_event(event),
                UiEvent::TranscriptionResult { text } => {
                    self.transcribe_pending = false;
                    if std::mem::take(&mut self.discard_pending) {
                        // Cancelled (Esc) after stop: drop the late result.
                        return;
                    }
                    // The live stream appended raw `stream_committed`; finalize
                    // returns post-processed text (filler/stutter removal,
                    // custom words), so replace exactly what was appended.
                    let committed = std::mem::take(&mut self.stream_committed);
                    if !committed.is_empty() && self.text.ends_with(&committed) {
                        self.text.truncate(self.text.len() - committed.len());
                        self.text.push_str(&text);
                    } else if !committed.is_empty()
                        && text.starts_with(&committed)
                        && text.len() > committed.len()
                    {
                        // Append-only session; finalize added a tail.
                        self.text.push_str(&text[committed.len()..]);
                    } else if !self.text.is_empty() && !text.is_empty() {
                        self.text.push('\n');
                        self.text.push_str(&text);
                    } else {
                        self.text.push_str(&text);
                    }
                    self.finish_streaming();
                    self.status = AppStatus::Ready;
                }
                UiEvent::TranscriptionError { error } => {
                    self.transcribe_pending = false;
                    let discarded = std::mem::take(&mut self.discard_pending);
                    if !discarded {
                        self.finish_streaming();
                        self.status = AppStatus::Error(error);
                    }
                }
            }
        }
    }

    fn finish_streaming(&mut self) {
        self.stream_finalized = true;
        self.stream_committed.clear();
        self.tentative.clear();
        self.stream_phase = None;
    }

    fn handle_core_event(&mut self, event: CoreEvent) {
        match event {
            CoreEvent::ModelsUpdated => {
                self.models = self.voice.model_mgr.get_available_models();
            }
            CoreEvent::StreamText {
                committed,
                tentative,
            } => {
                if self.stream_finalized {
                    return;
                }
                if committed.len() > self.stream_committed.len() {
                    if let Some(tail) = committed.strip_prefix(&self.stream_committed) {
                        self.text.push_str(tail);
                    }
                    self.stream_committed = committed;
                }
                self.tentative = tentative;
            }
            CoreEvent::StreamPhase { phase } => {
                if self.stream_finalized {
                    return;
                }
                self.stream_phase = Some(phase);
            }
            CoreEvent::ModelDownloadProgress {
                model_id,
                percentage,
                ..
            } => {
                self.downloading = Some((model_id.clone(), percentage));
                self.status = AppStatus::ModelDownloading { percentage };
            }
            CoreEvent::ModelDownloadComplete { .. } => {
                self.downloading = None;
                self.models = self.voice.model_mgr.get_available_models();
                if matches!(&self.status, AppStatus::ModelDownloading { .. }) {
                    self.status = AppStatus::Ready;
                }
            }
            CoreEvent::ModelDownloadCancelled { .. } => {
                self.downloading = None;
                self.status = AppStatus::Ready;
            }
            CoreEvent::ModelStateChanged {
                event_type, error, ..
            } => match event_type.as_str() {
                "loading_started" | "loading" => {
                    self.status = AppStatus::ModelLoading;
                }
                "loaded" | "loading_completed" | "load_complete" => {
                    self.status = AppStatus::Ready;
                }
                "error" | "load-failed" | "load_failed" => {
                    self.status = AppStatus::Error(
                        error.unwrap_or_else(|| "Model failed to load".to_string()),
                    );
                }
                _ => {}
            },
            CoreEvent::AudioLevels(levels) if self.recording_flag.load(Ordering::Relaxed) => {
                self.mic_levels = levels;
            }
            _ => {}
        }
    }

    fn process_hotkeys(&mut self) {
        while let Ok(hotkey) = self.hotkey_rx.try_recv() {
            match hotkey {
                Hotkey::ToggleRecording => self.toggle_recording(),
            }
        }
    }

    fn input_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)) {
            self.send_text(ctx);
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            if self.voice.audio_mgr.is_recording() {
                self.voice.cancel();
            }
            self.finish_streaming();
            self.status = AppStatus::Ready;
            self.last_recording_started = None;
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::Space)) {
            self.toggle_recording();
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma)) {
            self.settings_open = !self.settings_open;
        }
    }

    fn sync_status(&mut self) {
        if self.voice.audio_mgr.is_recording() {
            self.last_recording_started.get_or_insert_with(Instant::now);
            if !matches!(
                &self.status,
                AppStatus::ModelLoading | AppStatus::ModelDownloading { .. } | AppStatus::Error(_)
            ) {
                self.status = AppStatus::Recording;
            }
        } else if matches!(&self.status, AppStatus::Recording) {
            self.status = AppStatus::Transcribing;
            self.last_recording_started = None;
        }
    }
}

impl App for PromptClearApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_events();
        self.process_hotkeys();
        self.input_shortcuts(ctx);
        if self.send_confirmation.is_some()
            && ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Text(_))))
        {
            self.send_confirmation = None;
        }
        self.sync_status();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("topbar").show(ui, |ui| {
            ui::top_bar(ui, self);
        });
        egui::Panel::bottom("statusbar").show(ui, |ui| {
            ui::status_bar(ui, self);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            ui::editor(ui, self);
        });
        if self.settings_open {
            ui::settings_window(ui.ctx(), self);
        }

        match &self.status {
            AppStatus::Recording
            | AppStatus::Transcribing
            | AppStatus::ModelLoading
            | AppStatus::ModelDownloading { .. } => {
                ui.ctx().request_repaint_after(Duration::from_millis(250));
            }
            _ => {}
        }
    }
}
