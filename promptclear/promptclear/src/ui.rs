// PromptClear — offline voice notes. MIT.

use std::time::{Duration, Instant};

use egui_shadcn::{
    Badge, BadgeVariant, Button, ButtonVariant, Card, Collapsible, ComponentSize, Kbd, Label,
    LucideIcon, Progress, SelectValue, Separator, ShadcnThemeExt, Slider, StatusBar, Switch, Tabs,
    Textarea, Toolbar, Tooltip,
};
use handy_core::settings::{Accent, CornerRadius, ModelUnloadTimeout, ThemeMode};

use crate::app::{AppStatus, PromptClearApp};

pub fn top_bar(ui: &mut egui::Ui, app: &mut PromptClearApp) {
    Toolbar::new().dense().show(ui, |ui| {
        Label::new("PromptClear").muted().show(ui);
        Separator::vertical().show(ui);
        Label::new("Model").muted().show(ui);
        model_selector(ui, app);
        download_button(ui, app);
        Separator::vertical().show(ui);
        mic_button(ui, app);
        send_button(ui, app);
        Separator::vertical().show(ui);
        let settings = Button::new("Settings")
            .variant(ButtonVariant::Ghost)
            .icon(LucideIcon::Settings)
            .size(ComponentSize::Sm)
            .show(ui);
        Tooltip::new("Settings (Cmd+,)").show(&settings);
        if settings.clicked() {
            app.settings_open = true;
        }
    });
}

#[derive(Clone, PartialEq)]
struct ModelChoice {
    id: String,
    label: String,
}

impl std::fmt::Display for ModelChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// Selector label for a model. On-disk models get a "Local —" prefix so users
/// can pick them without downloading.
fn model_label(model: &handy_core::ModelInfo) -> String {
    let prefix = if is_locally_available(model) {
        "Local — "
    } else {
        ""
    };
    format!("{prefix}{} ({} MB)", model.name, model.size_mb)
}

/// True when the model resolves purely from local disk (PromptClear/Handy
/// model dirs, the shared HuggingFace cache, or a user-registered path) — i.e.
/// selectable without downloading. The OS speech engine is system-provided, so
/// it doesn't count.
fn is_locally_available(model: &handy_core::ModelInfo) -> bool {
    model.is_downloaded && !matches!(model.source, handy_core::ModelSource::OsSpeech)
}

fn model_choice(selected_id: &str, models: &[handy_core::ModelInfo]) -> ModelChoice {
    models
        .iter()
        .find(|model| model.id == selected_id)
        .map(|model| ModelChoice {
            id: model.id.clone(),
            label: model_label(model),
        })
        .unwrap_or_else(|| ModelChoice {
            id: selected_id.to_string(),
            label: "No model selected".to_string(),
        })
}

pub fn model_selector(ui: &mut egui::Ui, app: &mut PromptClearApp) {
    let choices: Vec<ModelChoice> = app
        .models
        .iter()
        .map(|model| ModelChoice {
            id: model.id.clone(),
            label: model_label(model),
        })
        .collect();
    if choices.is_empty() {
        Label::new("No models available").muted().show(ui);
        return;
    }
    let mut selected = model_choice(&app.app_settings.selected_model, &app.models);
    SelectValue::new(&mut selected, &choices)
        .width(200.0)
        .show(ui);
    if selected.id != app.app_settings.selected_model {
        app.app_settings.selected_model = selected.id;
        app.persist_settings();
    }
}

pub fn download_button(ui: &mut egui::Ui, app: &mut PromptClearApp) {
    let selected_id = app.app_settings.selected_model.clone();

    if let Some((model_id, percentage)) = app.downloading.clone() {
        if model_id == selected_id {
            ui.scope(|ui| {
                ui.set_min_width(140.0);
                ui.set_max_width(140.0);
                Progress::new((percentage / 100.0) as f32).show(ui);
            });
            Label::new(format!("{percentage:.0}%"))
                .muted()
                .size(ComponentSize::Sm)
                .show(ui);
        }
        return;
    }

    let Some(model) = app
        .models
        .iter()
        .find(|model| model.id == selected_id)
        .cloned()
    else {
        return;
    };
    if model.is_downloading || model.is_downloaded {
        return;
    }

    let download = Button::new("Download")
        .variant(ButtonVariant::Outline)
        .icon(LucideIcon::Download)
        .size(ComponentSize::Sm)
        .show(ui);
    Tooltip::new(format!("Download {} ({} MB)", model.name, model.size_mb)).show(&download);
    if download.clicked() {
        app.voice.start_download(&model.id);
        app.downloading = Some((model.id.clone(), 0.0));
        app.status = AppStatus::ModelDownloading { percentage: 0.0 };
    }
}

pub fn mic_button(ui: &mut egui::Ui, app: &mut PromptClearApp) {
    let recording = app.voice.audio_mgr.is_recording();
    let (text, icon, variant) = if recording {
        ("Stop", LucideIcon::Square, ButtonVariant::Default)
    } else {
        ("Dictate", LucideIcon::Mic, ButtonVariant::Outline)
    };
    let mic = Button::new(text)
        .icon(icon)
        .variant(variant)
        .size(ComponentSize::Sm)
        .show(ui);
    let hint = if recording {
        "Stop recording (Esc)"
    } else {
        "Start dictation (Alt+Space)"
    };
    Tooltip::new(hint).show(&mic);
    if mic.clicked() {
        app.toggle_recording();
    }
}

pub fn send_button(ui: &mut egui::Ui, app: &mut PromptClearApp) {
    let send = Button::new("Send")
        .icon(LucideIcon::Send)
        .variant(ButtonVariant::Default)
        .size(ComponentSize::Sm)
        .enabled(!app.text.is_empty())
        .show(ui);
    Tooltip::new("Copy to clipboard (Cmd+Enter)").show(&send);
    if send.clicked() {
        app.send_text(ui.ctx());
    }
}

pub fn editor(ui: &mut egui::Ui, app: &mut PromptClearApp) {
    let started = Instant::now();
    let theme = ui.ctx().shadcn_theme();

    Card::new().show(ui, |ui| {
        ui.horizontal(|ui| {
            Label::new("Prompt").show(ui);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                Kbd::new("Esc").show(ui);
                Label::new("cancel")
                    .muted()
                    .size(ComponentSize::Sm)
                    .show(ui);
                ui.add_space(6.0);
                Kbd::new("Cmd+Enter").show(ui);
                Label::new("send").muted().size(ComponentSize::Sm).show(ui);
                ui.add_space(6.0);
                Kbd::new("Alt+Space").show(ui);
                Label::new("dictate")
                    .muted()
                    .size(ComponentSize::Sm)
                    .show(ui);
            });
        });
        ui.add_space(6.0);

        let width = ui.available_width();
        let height = (ui.available_height() - 26.0).max(80.0);

        let response = if app.app_settings.spellcheck_enabled && !app.spellcheck_soft_disabled {
            // The spellcheck worker runs inside the dependency and can panic
            // on arbitrary user text; when it does, its results mutex is
            // poisoned and every subsequent frame would panic in
            // `take()` (egui_spellcheck 0.35, spell_check.rs). Catch the
            // panic here and permanently fall back to the plain editor so the
            // app survives.
            let spellcheck = run_spellchecked(ui, |ui| {
                egui_spellcheck::SpellCheckTextEdit::multiline(&mut app.text)
                    .id_salt("pc_editor")
                    .hint_text("Type or press Alt+Space to dictate…")
                    .desired_width(f32::INFINITY)
                    .show(ui)
                    .response
                    .response
            });
            match spellcheck {
                Ok(response) => response,
                Err(payload) => {
                    let message = payload
                        .downcast_ref::<&str>()
                        .copied()
                        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                        .unwrap_or("unknown panic");
                    log::error!("spellcheck editor panicked, disabling it: {message}");
                    app.spellcheck_soft_disabled = true;
                    app.user_notice =
                        Some("Spell check crashed and is off for this session".to_string());
                    plain_textarea(ui, app, width, height)
                }
            }
        } else {
            plain_textarea(ui, app, width, height)
        };
        if !app.editor_focus_requested {
            response.request_focus();
            app.editor_focus_requested = true;
        }

        if !app.tentative.is_empty() {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(&app.tentative)
                    .color(theme.muted_foreground)
                    .italics(),
            );
        }
    });

    app.spell_latency_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
}

/// The spellcheck-free editor: a plain shadcn textarea. Used when spell check
/// is off by setting or after the spellcheck path panicked once.
fn plain_textarea(
    ui: &mut egui::Ui,
    app: &mut PromptClearApp,
    width: f32,
    height: f32,
) -> egui::Response {
    Textarea::new(&mut app.text)
        .placeholder("Type or press Alt+Space to dictate…")
        .desired_width(width)
        .min_height(height)
        .show(ui)
}

/// Runs a spellcheck-editor pass inside `catch_unwind`.
///
/// `egui_spellcheck` runs its check on a background worker whose results are
/// exchanged through a mutex that the worker poisons when it panics on some
/// piece of user text; the UI thread then panics every frame via
/// `lock().expect(...)` (spell_check.rs). Catching the panic here lets the
/// caller switch to the plain editor before the next frame.
fn run_spellchecked(
    ui: &mut egui::Ui,
    render: impl FnOnce(&mut egui::Ui) -> egui::Response,
) -> Result<egui::Response, Box<dyn std::any::Any + Send>> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| render(ui)))
}

#[cfg(test)]
mod tests {
    use egui::{Context, RawInput};
    use handy_core::{EngineType, ModelInfo, ModelSource};

    use super::{is_locally_available, model_label, run_spellchecked};

    fn info(is_downloaded: bool, source: ModelSource) -> ModelInfo {
        ModelInfo {
            id: "m".to_string(),
            name: "My Model".to_string(),
            description: String::new(),
            filename: "m.gguf".to_string(),
            source,
            size_mb: 42,
            is_downloaded,
            is_downloading: false,
            partial_size: 0,
            is_directory: false,
            engine_type: EngineType::TranscribeCpp,
            accuracy_score: 0.0,
            speed_score: 0.0,
            supports_translation: false,
            is_recommended: false,
            supported_languages: Vec::new(),
            supports_language_selection: false,
            is_custom: true,
            supports_streaming: false,
            supports_language_detection: false,
        }
    }

    /// The catch_unwind guard must swallow a panic from the spellcheck path and
    /// leave the frame (and the next one) fully functional.
    #[test]
    fn spellcheck_panic_is_caught_and_frame_survives() {
        let ctx = Context::default();
        let mut seen_second_frame = false;
        let _ = ctx.run_ui(RawInput::default(), |ui| {
            let result = run_spellchecked(ui, |_ui| panic!("simulated spellcheck crash"));
            assert!(result.is_err(), "the panic must be caught, not propagated");
            let _ = ui.label("still alive");
        });
        // A second frame after the panic must render normally.
        let _ = ctx.run_ui(RawInput::default(), |ui| {
            let _ = ui.label("second frame");
            seen_second_frame = true;
        });
        assert!(seen_second_frame);
    }

    /// The panic payload must be downcastable for logging.
    #[test]
    fn panic_payload_is_readable() {
        let ctx = Context::default();
        let _ = ctx.run_ui(RawInput::default(), |ui| {
            let result = run_spellchecked(ui, |_ui| panic!("boom"));
            let err = result.unwrap_err();
            let message = err
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| err.downcast_ref::<String>().map(String::as_str))
                .unwrap_or("unknown panic");
            assert_eq!(message, "boom");
        });
    }

    /// On-disk models (PromptClear/Handy dirs, HF cache, registered path) get
    /// the "Local —" prefix; the OS speech engine and undownloaded models do not.
    #[test]
    fn model_label_prefixes_locally_available_models() {
        let local = info(true, ModelSource::Local);
        assert!(model_label(&local).starts_with("Local — My Model"));

        let cached = info(
            true,
            ModelSource::HuggingFace {
                repo_id: "org/repo".to_string(),
                revision: "main".to_string(),
            },
        );
        assert!(model_label(&cached).starts_with("Local — My Model"));

        let registered = info(true, ModelSource::LocalPath);
        assert!(model_label(&registered).starts_with("Local — My Model"));

        let os_speech = info(true, ModelSource::OsSpeech);
        assert_eq!(model_label(&os_speech), "My Model (42 MB)");

        let remote = info(
            false,
            ModelSource::Url {
                url: "https://example.com".to_string(),
                sha256: None,
            },
        );
        assert_eq!(model_label(&remote), "My Model (42 MB)");
        assert!(is_locally_available(&local));
        assert!(!is_locally_available(&os_speech));
        assert!(!is_locally_available(&remote));
    }
}

pub fn status_bar(ui: &mut egui::Ui, app: &mut PromptClearApp) {
    let theme = ui.ctx().shadcn_theme();
    let recording = app.voice.audio_mgr.is_recording();

    StatusBar::new().show(ui, |ui| {
        let status_text = match &app.status {
            AppStatus::Ready => "Ready".to_string(),
            AppStatus::ModelLoading => "Loading model…".to_string(),
            AppStatus::ModelDownloading { percentage, .. } => {
                format!("Downloading… {percentage:.0}%")
            }
            AppStatus::Recording => {
                let seconds = app
                    .last_recording_started
                    .map(|started| started.elapsed().as_secs())
                    .unwrap_or(0);
                format!("Recording… {seconds}s")
            }
            AppStatus::Transcribing => "Transcribing…".to_string(),
            AppStatus::Error(_) => "Error".to_string(),
        };
        let variant = match &app.status {
            AppStatus::Ready => BadgeVariant::Outline,
            AppStatus::Error(_) => BadgeVariant::Destructive,
            AppStatus::Recording => BadgeVariant::Default,
            _ => BadgeVariant::Secondary,
        };
        let status = Badge::new(status_text).variant(variant).show(ui);
        if let AppStatus::Error(error) = &app.status {
            Tooltip::new(error.clone()).show(&status);
        }

        if let Some(phase) = &app.stream_phase {
            Separator::vertical().show(ui);
            Badge::new(phase).variant(BadgeVariant::Secondary).show(ui);
        }
        Separator::vertical().show(ui);
        let spellcheck_active =
            app.app_settings.spellcheck_enabled && !app.spellcheck_soft_disabled;
        Badge::new(if spellcheck_active {
            "Spell: on"
        } else {
            "Spell: off"
        })
        .variant(BadgeVariant::Outline)
        .show(ui);
        if spellcheck_active {
            if let Some(ms) = app.spell_latency_ms {
                Separator::vertical().show(ui);
                Label::new(format!("Spellcheck {ms:.0} ms"))
                    .muted()
                    .show(ui);
            }
        }
        Separator::vertical().show(ui);
        Label::new(format!("{} words", app.text.split_whitespace().count()))
            .muted()
            .show(ui);
        if recording {
            Separator::vertical().show(ui);
            level_bars(ui, &app.mic_levels, theme.primary);
        }
        if let Some(confirmation) = &app.send_confirmation {
            Separator::vertical().show(ui);
            ui.label(egui::RichText::new(confirmation).color(egui::Color32::from_rgb(80, 180, 90)));
        }
        if let Some(notice) = &app.user_notice {
            Separator::vertical().show(ui);
            ui.label(egui::RichText::new(notice).color(egui::Color32::from_rgb(220, 160, 60)));
        }
    });
}

fn level_bars(ui: &mut egui::Ui, levels: &[f32], accent: egui::Color32) {
    if levels.is_empty() {
        return;
    }
    let (rect, _) = ui.allocate_exact_size(egui::vec2(64.0, 14.0), egui::Sense::hover());
    let painter = ui.painter();
    let bar_width = rect.width() / levels.len() as f32;
    for (index, level) in levels.iter().enumerate() {
        let level = level.clamp(0.0, 1.0);
        let height = (rect.height() * level).max(1.0);
        let x = rect.left() + index as f32 * bar_width;
        let bar = egui::Rect::from_min_size(
            egui::pos2(x, rect.bottom() - height),
            egui::vec2((bar_width - 1.0).max(1.0), height),
        );
        painter.rect_filled(bar, egui::CornerRadius::same(2), accent);
    }
}

pub fn settings_window(ctx: &egui::Context, app: &mut PromptClearApp) {
    let mut open = app.settings_open;
    let theme = ctx.shadcn_theme();

    egui::Window::new("Settings")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(520.0)
        .frame(
            egui::Frame::NONE
                .fill(theme.background)
                .inner_margin(egui::Margin::same(16))
                .corner_radius(egui::CornerRadius::same(theme.radius.round() as u8 + 2))
                .stroke(egui::Stroke::new(1.0, theme.border)),
        )
        .show(ctx, |ui| {
            let tab_id = egui::Id::new("settings_tab");
            let mut tab = ui.data_mut(|data| *data.get_temp_mut_or_default::<usize>(tab_id));
            Tabs::new(vec![
                "Model".to_string(),
                "Behavior".to_string(),
                "Appearance".to_string(),
            ])
            .show(ui, &mut tab, |ui, index| match index {
                0 => model_tab(ui, app),
                1 => behavior_tab(ui, app),
                _ => appearance_tab(ui, app),
            });
            ui.data_mut(|data| data.insert_temp(tab_id, tab));

            ui.add_space(10.0);
            Separator::horizontal().show(ui);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                Label::new("Shortcuts").muted().show(ui);
                ui.add_space(4.0);
                Kbd::new("Alt+Space").show(ui);
                Label::new("dictate")
                    .muted()
                    .size(ComponentSize::Sm)
                    .show(ui);
                ui.add_space(6.0);
                Kbd::new("Cmd+Enter").show(ui);
                Label::new("send").muted().size(ComponentSize::Sm).show(ui);
                ui.add_space(6.0);
                Kbd::new("Cmd+Space").show(ui);
                Label::new("toggle")
                    .muted()
                    .size(ComponentSize::Sm)
                    .show(ui);
                ui.add_space(6.0);
                Kbd::new("Esc").show(ui);
                Label::new("cancel")
                    .muted()
                    .size(ComponentSize::Sm)
                    .show(ui);
            });
        });
    app.settings_open = open;
}

/// Re-run the local discovery scans at most this often while the Model tab is
/// open, so newly placed model files surface without a restart. The manager
/// coalesces concurrent scans (single-flight) on a background thread.
const LOCAL_RESCAN_INTERVAL: Duration = Duration::from_secs(5);

fn model_tab(ui: &mut egui::Ui, app: &mut PromptClearApp) {
    if app
        .last_local_scan
        .is_none_or(|at| at.elapsed() > LOCAL_RESCAN_INTERVAL)
    {
        app.last_local_scan = Some(Instant::now());
        app.voice.rescan_local_models();
    }

    Label::new("Model").muted().show(ui);
    ui.add_space(4.0);
    model_selector(ui, app);
    download_button(ui, app);
    ui.add_space(6.0);
    advanced_section(ui, app);
}

fn advanced_section(ui: &mut egui::Ui, app: &mut PromptClearApp) {
    let advanced_id = egui::Id::new("model_tab_advanced");
    let mut open = ui.data_mut(|data| *data.get_temp_mut_or_default::<bool>(advanced_id));
    Collapsible::new("Advanced").show(ui, &mut open, |ui| {
        local_scan_controls(ui, app);
        ui.add_space(8.0);
        Separator::horizontal().show(ui);
        ui.add_space(8.0);
        Label::new("Custom model").muted().show(ui);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let pick = Button::new("Choose custom GGUF/ONNX…")
                .variant(ButtonVariant::Outline)
                .icon(LucideIcon::FolderOpen)
                .show(ui);
            if pick.clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Models", &["gguf", "bin", "onnx"])
                    .pick_file()
                {
                    app.voice.register_custom_model(path);
                }
            }
            let has_custom = app.models.iter().any(|model| model.is_custom);
            if has_custom {
                let remove = Button::new("Remove custom model")
                    .variant(ButtonVariant::Ghost)
                    .icon(LucideIcon::Trash2)
                    .show(ui);
                if remove.clicked() {
                    if let Some(custom) = app.models.iter().find(|model| model.is_custom) {
                        let id = custom.id.clone();
                        app.voice.delete_model(&id);
                    }
                }
            }
        });
    });
    ui.data_mut(|data| data.insert_temp(advanced_id, open));
}

fn local_scan_controls(ui: &mut egui::Ui, app: &mut PromptClearApp) {
    Label::new("Local model scanning").muted().show(ui);
    ui.add_space(4.0);
    let scanning = app.voice.model_mgr.is_rescanning();
    ui.horizontal(|ui| {
        let scan = Button::new("Scan local folders")
            .variant(ButtonVariant::Outline)
            .icon(LucideIcon::RefreshCw)
            .enabled(!scanning)
            .show(ui);
        Tooltip::new(
            "Re-scan the PromptClear/Handy model folders and the HuggingFace cache for new .gguf/.bin files",
        )
        .show(&scan);
        if scan.clicked() {
            app.last_local_scan = Some(Instant::now());
            app.voice.rescan_local_models();
        }
        if scanning {
            Label::new("Scanning…")
                .muted()
                .size(ComponentSize::Sm)
                .show(ui);
        }
    });
    let count = app
        .models
        .iter()
        .filter(|model| is_locally_available(model))
        .count();
    Label::new(format!(
        "{count} model(s) available locally — PromptClear/Handy folders + HuggingFace cache"
    ))
    .muted()
    .size(ComponentSize::Sm)
    .show(ui);
}

fn behavior_tab(ui: &mut egui::Ui, app: &mut PromptClearApp) {
    unload_timeout_selector(ui, app);
    language_selector(ui, app);
    custom_words_editor(ui, app);

    Label::new("Correction threshold").muted().show(ui);
    let threshold = Slider::new(&mut app.app_settings.word_correction_threshold, 0.0..=1.0)
        .width(ui.available_width())
        .show(ui);
    if threshold.changed() {
        app.persist_settings();
    }

    ui.add_space(8.0);
    let streaming = Switch::new(&mut app.app_settings.streaming_enabled)
        .label("Streaming dictation (live text)")
        .show(ui);
    if streaming.changed() {
        app.persist_settings();
    }
    let spellcheck = Switch::new(&mut app.app_settings.spellcheck_enabled)
        .label("Spell check")
        .show(ui);
    if spellcheck.changed() {
        if app.app_settings.spellcheck_enabled {
            egui_spellcheck::prewarm();
        }
        app.persist_settings();
    }
}

#[derive(Clone, PartialEq)]
struct TimeoutOption {
    value: ModelUnloadTimeout,
    label: &'static str,
}

impl std::fmt::Display for TimeoutOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label)
    }
}

const TIMEOUT_OPTIONS: [TimeoutOption; 4] = [
    TimeoutOption {
        value: ModelUnloadTimeout::After5Minutes,
        label: "5 minutes",
    },
    TimeoutOption {
        value: ModelUnloadTimeout::After10Minutes,
        label: "10 minutes",
    },
    TimeoutOption {
        value: ModelUnloadTimeout::After30Minutes,
        label: "30 minutes",
    },
    TimeoutOption {
        value: ModelUnloadTimeout::Never,
        label: "Never",
    },
];

fn unload_timeout_selector(ui: &mut egui::Ui, app: &mut PromptClearApp) {
    Label::new("Model unload timeout").muted().show(ui);
    let mut selected = TIMEOUT_OPTIONS
        .iter()
        .find(|option| option.value == app.app_settings.model_unload_timeout)
        .cloned()
        .unwrap_or_else(|| TIMEOUT_OPTIONS[3].clone());
    SelectValue::new(&mut selected, &TIMEOUT_OPTIONS)
        .width(ui.available_width())
        .show(ui);
    if selected.value != app.app_settings.model_unload_timeout {
        app.app_settings.model_unload_timeout = selected.value;
        app.persist_settings();
    }
}

fn language_selector(ui: &mut egui::Ui, app: &mut PromptClearApp) {
    let mut languages: Vec<String> = vec!["auto".to_string()];
    if let Some(model) = app
        .models
        .iter()
        .find(|model| model.id == app.app_settings.selected_model)
    {
        for language in &model.supported_languages {
            if !languages.contains(language) {
                languages.push(language.clone());
            }
        }
    }

    Label::new("Language").muted().show(ui);
    let mut selected = if app.app_settings.selected_language.is_empty() {
        "auto".to_string()
    } else {
        app.app_settings.selected_language.clone()
    };
    SelectValue::new(&mut selected, &languages)
        .width(ui.available_width())
        .show(ui);
    if selected != app.app_settings.selected_language {
        app.app_settings.selected_language = selected;
        app.persist_settings();
    }
}

fn custom_words_editor(ui: &mut egui::Ui, app: &mut PromptClearApp) {
    Label::new("Custom words (one per line)").muted().show(ui);
    let mut words = app.app_settings.custom_words.join("\n");
    let width = ui.available_width();
    let response = Textarea::new(&mut words)
        .placeholder("Names, terms, jargon…")
        .desired_width(width)
        .min_height(72.0)
        .show(ui);
    if response.changed() {
        app.app_settings.custom_words = words
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect();
        app.persist_settings();
    }
}

fn appearance_tab(ui: &mut egui::Ui, app: &mut PromptClearApp) {
    let mut changed = false;

    Label::new("Theme mode").muted().show(ui);
    ui.horizontal(|ui| {
        for mode in ThemeMode::ALL {
            let icon = match mode {
                ThemeMode::Dark => LucideIcon::Moon,
                ThemeMode::Light => LucideIcon::Sun,
            };
            let button = Button::new(mode.label())
                .icon(icon)
                .variant(ButtonVariant::Outline)
                .selected(mode == app.app_settings.theme_mode)
                .show(ui);
            if button.clicked() && mode != app.app_settings.theme_mode {
                app.app_settings.theme_mode = mode;
                changed = true;
            }
        }
    });

    ui.add_space(8.0);
    Label::new("Accent color").muted().show(ui);
    accent_picker(ui, app, &mut changed);

    ui.add_space(8.0);
    Label::new("Corner radius").muted().show(ui);
    ui.horizontal(|ui| {
        for radius in CornerRadius::ALL {
            let button = Button::new(radius.label())
                .variant(ButtonVariant::Outline)
                .selected(radius == app.app_settings.corner_radius)
                .show(ui);
            if button.clicked() && radius != app.app_settings.corner_radius {
                app.app_settings.corner_radius = radius;
                changed = true;
            }
        }
    });

    if changed {
        app.persist_settings();
        app.apply_theme(ui.ctx());
    }
}

fn accent_picker(ui: &mut egui::Ui, app: &mut PromptClearApp, changed: &mut bool) {
    ui.horizontal_wrapped(|ui| {
        for accent in Accent::ALL {
            let selected = accent == app.app_settings.accent_color;
            let color = crate::theme::accent_swatch(accent, app.app_settings.theme_mode);
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::click());
            if ui.is_rect_visible(rect) {
                let painter = ui.painter();
                painter.circle_filled(rect.center(), 9.0, color);
                if selected {
                    painter.circle_stroke(
                        rect.center(),
                        11.0,
                        egui::Stroke::new(2.0, ui.ctx().shadcn_theme().ring),
                    );
                } else if response.hovered() {
                    painter.circle_stroke(
                        rect.center(),
                        11.0,
                        egui::Stroke::new(1.0, ui.ctx().shadcn_theme().border),
                    );
                }
            }
            Tooltip::new(accent.label()).show(&response);
            if response.clicked() && !selected {
                app.app_settings.accent_color = accent;
                *changed = true;
            }
        }
    });
}
