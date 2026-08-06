// PromptClear — offline voice notes. MIT.

use std::path::{Path, PathBuf};

/// Make sure the Silero VAD model is in place at the core's expected path.
///
/// The app bundles `silero_vad_v4.onnx`; this copies it into the user's data
/// directory on first run. If the bundled copy can't be found the app still
/// starts — recording simply fails later with a clear error message.
pub fn ensure_vad_model() -> anyhow::Result<()> {
    let destination = handy_core::paths::default_vad_model_path();
    if destination.is_file() {
        return Ok(());
    }

    let mut found = None;
    for candidate in bundled_vad_candidates() {
        let path = candidate.join("silero_vad_v4.onnx");
        if path.is_file() {
            found = Some(path);
            break;
        }
    }

    let Some(source) = found else {
        log::warn!(
            "bundled silero_vad_v4.onnx not found (searched {:?}); \
             recording will fail until a VAD model is present at {}",
            bundled_vad_candidates(),
            destination.display()
        );
        return Ok(());
    };

    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(&source, &destination)?;
    log::info!("copied bundled VAD model to {}", destination.display());
    Ok(())
}

fn bundled_vad_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(dir) = std::env::var_os("PROMPTCLEAR_RESOURCES_DIR") {
        candidates.push(PathBuf::from(dir));
    }

    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
    {
        candidates.push(exe_dir.join("resources"));
    }

    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        candidates.push(PathBuf::from(manifest_dir).join("resources"));
    }

    // Compile-time manifest dir: `cargo run` sets the env var above, but
    // direct binary runs (and packaged builds) do not.
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources"));

    candidates
}
