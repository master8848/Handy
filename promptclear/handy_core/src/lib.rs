//! handy_core — offline audio/VAD/transcription core extracted from
//! [Handy](https://github.com/cjpais/Handy) (MIT, (c) 2025 CJ Pais).
//!
//! No Tauri, no UI: recording, VAD, resampling, model management (download +
//! custom local paths) and transcription. UI frameworks plug in via
//! [`event::EventSink`] and [`settings::SettingsStore`].

pub mod audio;
pub mod audio_manager;
pub mod catalog;
pub mod constants;
pub mod download;
pub mod event;
pub mod gguf_meta;
pub mod model;
pub mod model_capabilities;
pub mod paths;
pub mod settings;
pub mod text;
pub mod transcription;
pub mod utils;
pub mod vad;

pub use audio::{
    get_cpal_host, is_microphone_access_denied, is_no_input_device_error, list_input_devices,
    list_output_devices, read_wav_samples, save_wav_file, verify_wav_file, AudioRecorder,
    CpalDeviceInfo, VadPolicy,
};
pub use audio_manager::AudioRecordingManager;
pub use event::{CoreEvent, EventSink};
pub use model::{DownloadProgress, EngineType, ModelInfo, ModelManager, ModelSource};
pub use settings::{AppSettings, JsonSettingsStore, SettingsStore};
pub use text::{apply_custom_words, filter_transcription_output};
pub use transcription::TranscriptionManager;
pub use utils::is_windows_x64_emulated_on_arm64;
pub use vad::{SileroVad, VadFrame, VoiceActivityDetector};
