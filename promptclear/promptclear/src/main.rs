// PromptClear — offline voice notes. MIT.

use std::io::Write;
use std::sync::{mpsc, Arc};
use std::time::{SystemTime, UNIX_EPOCH};

use handy_core::paths::{
    default_data_dir, default_models_dir, default_settings_path, default_vad_model_path,
};
use handy_core::settings::{JsonSettingsStore, SettingsStore};
use handy_core::{AudioRecordingManager, EventSink, ModelManager, TranscriptionManager};
use promptclear::app::PromptClearApp;
use promptclear::events::{EventBus, Hotkey};
use promptclear::voice::VoicePipeline;

/// Installs a panic hook that appends every panic to `panic.log` in the data
/// dir (timestamp + thread + backtrace), in addition to the default stderr
/// reporting. Panics are NOT suppressed — the hook exists for diagnosis, and
/// app-level `catch_unwind` guards still do the crash containment.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_hook(info);

        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("<unnamed>");
        let backtrace = std::backtrace::Backtrace::force_capture();
        let entry =
            format!("[{seconds}] panic in thread '{thread_name}': {info}\n{backtrace}\n---\n");

        let path = default_data_dir().join("panic.log");
        let wrote = std::fs::create_dir_all(default_data_dir())
            .and_then(|_| {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
            })
            .and_then(|mut file| file.write_all(entry.as_bytes()));
        if let Err(error) = wrote {
            eprintln!("failed to write panic log to {}: {error}", path.display());
        }
    }));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    install_panic_hook();
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
