use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Accelerator preference for the transcribe.cpp path.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TranscribeAcceleratorSetting {
    #[serde(rename = "ort")]
    Ort,
    #[serde(rename = "cpp")]
    Cpp,
    #[serde(rename = "auto")]
    #[default]
    Auto,
}

/// Controls how the given acceleration setting is applied when ORT is available.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum OrtAcceleratorSetting {
    #[serde(rename = "cpu")]
    #[default]
    Cpu,
    #[serde(rename = "cpu-mem-optimize")]
    CpuMemOptimize,
    #[serde(rename = "cuda")]
    Cuda,
    #[serde(rename = "cuda-tensorrt")]
    CudaTensorrt,
    #[serde(rename = "directml")]
    DirectMl,
    #[serde(rename = "coreml")]
    CoreMl,
    #[serde(rename = "auto")]
    Auto,
}

/// When to automatically unload the model after inactivity.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ModelUnloadTimeout {
    #[serde(rename = "5min")]
    After5Minutes,
    #[serde(rename = "10min")]
    After10Minutes,
    #[serde(rename = "30min")]
    After30Minutes,
    #[serde(rename = "never")]
    #[default]
    Never,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum MicrophoneMode {
    #[default]
    OnDemand,
    AlwaysOn,
}

/// UI color scheme preference.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

impl ThemeMode {
    pub const ALL: [ThemeMode; 2] = [ThemeMode::Dark, ThemeMode::Light];

    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::Dark => "Dark",
            ThemeMode::Light => "Light",
        }
    }
}

/// Brand accent color. `NeutralGray` is the monochrome "gray mode".
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Accent {
    #[default]
    HandyPink,
    NeutralGray,
    Blue,
    Sky,
    Cyan,
    Teal,
    Green,
    Lime,
    Amber,
    Orange,
    Red,
    Violet,
    Purple,
}

impl Accent {
    pub const ALL: [Accent; 13] = [
        Accent::HandyPink,
        Accent::NeutralGray,
        Accent::Blue,
        Accent::Sky,
        Accent::Cyan,
        Accent::Teal,
        Accent::Green,
        Accent::Lime,
        Accent::Amber,
        Accent::Orange,
        Accent::Red,
        Accent::Violet,
        Accent::Purple,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Accent::HandyPink => "Handy Pink",
            Accent::NeutralGray => "Gray",
            Accent::Blue => "Blue",
            Accent::Sky => "Sky",
            Accent::Cyan => "Cyan",
            Accent::Teal => "Teal",
            Accent::Green => "Green",
            Accent::Lime => "Lime",
            Accent::Amber => "Amber",
            Accent::Orange => "Orange",
            Accent::Red => "Red",
            Accent::Violet => "Violet",
            Accent::Purple => "Purple",
        }
    }
}

/// Corner radius preset, mapped onto the theme's global radius field.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CornerRadius {
    #[default]
    Default,
    Sharp,
    Rounded,
    VeryRounded,
}

impl CornerRadius {
    pub const ALL: [CornerRadius; 4] = [
        CornerRadius::Sharp,
        CornerRadius::Default,
        CornerRadius::Rounded,
        CornerRadius::VeryRounded,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CornerRadius::Sharp => "Sharp",
            CornerRadius::Default => "Default",
            CornerRadius::Rounded => "Rounded",
            CornerRadius::VeryRounded => "Very rounded",
        }
    }
}

/// The subset of Handy's `AppSettings` that the core consumes, plus the new
/// `custom_model_path` option. Field names match Handy's so ported code keeps
/// working unchanged.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppSettings {
    #[serde(default)]
    pub selected_model: String,
    /// Absolute path to a local GGUF/ONNX model file (PromptClear extension).
    #[serde(default)]
    pub custom_model_path: Option<String>,
    #[serde(default)]
    pub selected_language: String,
    #[serde(default)]
    pub translate_to_english: bool,
    #[serde(default)]
    pub custom_words: Vec<String>,
    #[serde(default)]
    pub word_correction_threshold: f64,
    #[serde(default)]
    pub app_language: String,
    #[serde(default)]
    pub custom_filler_words: Option<Vec<String>>,
    #[serde(default)]
    pub model_unload_timeout: ModelUnloadTimeout,
    #[serde(default)]
    pub transcribe_accelerator: TranscribeAcceleratorSetting,
    #[serde(default)]
    pub ort_accelerator: OrtAcceleratorSetting,
    #[serde(default)]
    pub transcribe_gpu_device: i32,
    #[serde(default)]
    pub microphone_mode: MicrophoneMode,
    #[serde(default)]
    pub selected_microphone: Option<String>,
    #[serde(default)]
    pub lazy_stream_close: bool,
    #[serde(default)]
    pub extra_recording_buffer_ms: u64,
    /// Live streaming dictation (committed text streams into the editor while
    /// speaking). Defaults to true, matching Handy.
    #[serde(default)]
    pub streaming_enabled: bool,
    /// Opt-in spelling/grammar checking of the editor. Defaults to false:
    /// dictation should not pay the checker cost by default.
    #[serde(default)]
    pub spellcheck_enabled: bool,
    /// UI color scheme preference.
    #[serde(default)]
    pub theme_mode: ThemeMode,
    /// Brand accent color; `NeutralGray` is the monochrome gray mode.
    #[serde(default)]
    pub accent_color: Accent,
    /// Corner radius preset for the whole UI.
    #[serde(default)]
    pub corner_radius: CornerRadius,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            selected_model: String::new(),
            custom_model_path: None,
            selected_language: String::new(),
            translate_to_english: false,
            custom_words: Vec::new(),
            word_correction_threshold: 0.0,
            app_language: String::new(),
            custom_filler_words: None,
            model_unload_timeout: ModelUnloadTimeout::Never,
            transcribe_accelerator: TranscribeAcceleratorSetting::Auto,
            ort_accelerator: OrtAcceleratorSetting::Cpu,
            transcribe_gpu_device: 0,
            microphone_mode: MicrophoneMode::OnDemand,
            selected_microphone: None,
            lazy_stream_close: false,
            extra_recording_buffer_ms: 0,
            streaming_enabled: true,
            spellcheck_enabled: false,
            theme_mode: ThemeMode::Dark,
            accent_color: Accent::HandyPink,
            corner_radius: CornerRadius::Default,
        }
    }
}

/// Seconds until automatic model unload, or `None` when never.
pub fn model_unload_timeout_seconds(timeout: ModelUnloadTimeout) -> Option<u64> {
    match timeout {
        ModelUnloadTimeout::After5Minutes => Some(5 * 60),
        ModelUnloadTimeout::After10Minutes => Some(10 * 60),
        ModelUnloadTimeout::After30Minutes => Some(30 * 60),
        ModelUnloadTimeout::Never => None,
    }
}

/// Persistence abstraction. The UI supplies a JSON-file-based store.
pub trait SettingsStore: Send + Sync {
    fn load(&self) -> Result<AppSettings>;
    fn save(&self, settings: &AppSettings) -> Result<()>;
}

/// Default store: pretty-printed JSON at a fixed path.
pub struct JsonSettingsStore {
    path: PathBuf,
}

impl JsonSettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl SettingsStore for JsonSettingsStore {
    fn load(&self) -> Result<AppSettings> {
        if !self.path.exists() {
            return Ok(AppSettings::default());
        }
        let data = std::fs::read_to_string(&self.path)?;
        let settings: AppSettings = serde_json::from_str(&data)?;
        Ok(settings)
    }

    fn save(&self, settings: &AppSettings) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(settings)?;
        std::fs::write(&self.path, data)?;
        Ok(())
    }
}

/// Shared shims, API-compatible with Handy's `get_settings`/`write_settings`.
pub fn get_settings(store: &dyn SettingsStore) -> AppSettings {
    store.load().unwrap_or_else(|e| {
        log::warn!("failed to load settings: {e}");
        AppSettings::default()
    })
}

pub fn write_settings(store: &dyn SettingsStore, settings: &AppSettings) -> Result<()> {
    store.save(settings)
}
