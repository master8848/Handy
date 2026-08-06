use std::path::PathBuf;

/// Override for tests / portable installs.
pub fn data_dir_override() -> Option<PathBuf> {
    std::env::var_os("PROMPTCLEAR_DATA_DIR").map(PathBuf::from)
}

pub fn default_data_dir() -> PathBuf {
    if let Some(p) = data_dir_override() {
        return p;
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("PromptClear")
}

pub fn default_models_dir() -> PathBuf {
    default_data_dir().join("models")
}

pub fn default_settings_path() -> PathBuf {
    default_data_dir().join("settings.json")
}

pub fn default_vad_model_path() -> PathBuf {
    default_models_dir().join("silero_vad_v4.onnx")
}
