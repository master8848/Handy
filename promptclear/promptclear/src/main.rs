// PromptClear — offline voice notes. MIT.

use std::sync::{mpsc, Arc};

use handy_core::paths::{default_models_dir, default_settings_path, default_vad_model_path};
use handy_core::settings::{JsonSettingsStore, SettingsStore};
use handy_core::{AudioRecordingManager, EventSink, ModelManager, TranscriptionManager};
use promptclear::app::PromptClearApp;
use promptclear::events::{EventBus, Hotkey};
use promptclear::voice::VoicePipeline;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    promptclear::paths::ensure_vad_model()?;

    let store: Arc<dyn SettingsStore> = Arc::new(JsonSettingsStore::new(default_settings_path()));
    let bus = EventBus::new();

    let (hotkey_tx, hotkey_rx) = mpsc::channel::<Hotkey>();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([920.0, 640.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "PromptClear",
        native_options,
        Box::new(move |cc: &eframe::CreationContext| {
            egui_shadcn::setup_fonts(&cc.egui_ctx);
            let initial_settings = handy_core::settings::get_settings(store.as_ref());
            promptclear::theme::apply(
                &cc.egui_ctx,
                initial_settings.theme_mode,
                initial_settings.accent_color,
                initial_settings.corner_radius,
            );

            let recording_flag: Arc<std::sync::atomic::AtomicBool> =
                Arc::new(std::sync::atomic::AtomicBool::new(false));
            let sink: Arc<dyn EventSink> =
                Arc::new(bus.sink(cc.egui_ctx.clone(), recording_flag.clone()));

            let model_mgr = Arc::new(ModelManager::new(
                default_models_dir(),
                store.clone(),
                sink.clone(),
            )?);
            let transcription_mgr = Arc::new(TranscriptionManager::new(
                store.clone(),
                sink.clone(),
                model_mgr.clone(),
            )?);
            let stream_router = transcription_mgr.stream_router();
            let audio_mgr = Arc::new(AudioRecordingManager::new(
                store.clone(),
                sink.clone(),
                default_vad_model_path(),
                stream_router,
            ));

            promptclear::hotkey::spawn(hotkey_tx, cc.egui_ctx.clone());

            let voice = VoicePipeline::new(audio_mgr, transcription_mgr, model_mgr, bus.tx.clone());

            Ok(Box::new(PromptClearApp::new(
                bus,
                hotkey_rx,
                voice,
                store,
                recording_flag,
            )))
        }),
    )?;

    Ok(())
}
