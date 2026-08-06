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

/// Legacy Handy app models directory (`com.pais.handy/models` under the
/// platform data dir: `~/Library/Application Support` on macOS,
/// `~/.local/share` on Linux, `%APPDATA%\Roaming` on Windows). PromptClear
/// scans it so models the old app downloaded show up without copying.
pub fn handy_models_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("com.pais.handy").join("models"))
}

pub fn default_settings_path() -> PathBuf {
    default_data_dir().join("settings.json")
}

pub fn default_vad_model_path() -> PathBuf {
    default_models_dir().join("silero_vad_v4.onnx")
}
