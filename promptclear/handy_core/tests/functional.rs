//! Headless end-to-end tests for the handy_core voice pipeline: settings
//! persistence, VAD preload, real model loading, and actual transcription.
//!
//! No GUI, no microphone. Resource-dependent tests (VAD onnx, GGUF model,
//! speech wav) auto-skip with a `SKIP:` eprintln when the resource is absent,
//! so this file is CI-safe on machines with no models.
//!
//! Run with `cargo test -p handy_core --test functional -- --test-threads=1`
//! (model loads are RAM/CPU heavy and the shared harness must not be hit
//! concurrently).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use handy_core::constants::WHISPER_SAMPLE_RATE;
use handy_core::event::{CoreEvent, EventSink};
use handy_core::settings::{
    Accent, AppSettings, CornerRadius, JsonSettingsStore, MicrophoneMode, ModelUnloadTimeout,
    OrtAcceleratorSetting, SettingsStore, ThemeMode, TranscribeAcceleratorSetting,
};
use handy_core::vad::{SileroVad, VoiceActivityDetector};
use handy_core::{AudioRecordingManager, ModelManager, TranscriptionManager};

/// Where the silero VAD onnx lives in this workspace; falls back to the
/// crate's default location under the app data dir (see resolve_vad_path).
const SILERO_VAD_CANDIDATES: [&str; 2] = [
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../promptclear/resources/silero_vad_v4.onnx"
    ),
    "/Users/apple/code/Handy/promptclear/promptclear/resources/silero_vad_v4.onnx",
];

/// How long to wait for a multi-GB GGUF to load (30-120s is typical; be generous).
const MODEL_LOAD_TIMEOUT: Duration = Duration::from_secs(180);
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/* ────────────────────────────────────────────────────────────────────────── */
/* test 1: settings persistence                                             */
/* ────────────────────────────────────────────────────────────────────────── */

/// Unique scratch dir under `std::env::temp_dir()` (no external crates).
fn unique_temp_dir(prefix: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "{prefix}_{}_{}_{}",
        std::process::id(),
        nanos,
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn settings_roundtrip() {
    let dir = unique_temp_dir("handy_core_settings_test");
    let store = JsonSettingsStore::new(dir.join("settings.json"));

    let s = AppSettings {
        selected_model: "ggml-small.bin".to_string(),
        custom_model_path: Some("/tmp/custom-whisper.gguf".to_string()),
        selected_language: "en".to_string(),
        translate_to_english: true,
        custom_words: vec!["handy".to_string(), "promptclear".to_string()],
        word_correction_threshold: 0.87,
        app_language: "de".to_string(),
        custom_filler_words: Some(vec!["um".to_string(), "like".to_string()]),
        model_unload_timeout: ModelUnloadTimeout::After10Minutes,
        transcribe_accelerator: TranscribeAcceleratorSetting::Cpp,
        ort_accelerator: OrtAcceleratorSetting::CoreMl,
        transcribe_gpu_device: 1,
        microphone_mode: MicrophoneMode::AlwaysOn,
        selected_microphone: Some("Test Mic".to_string()),
        lazy_stream_close: true,
        extra_recording_buffer_ms: 250,
        streaming_enabled: false,
        spellcheck_enabled: true,
        theme_mode: ThemeMode::Light,
        accent_color: Accent::Teal,
        corner_radius: CornerRadius::Rounded,
    };

    store.save(&s).expect("persist settings");
    let disk = dir.join("settings.json");
    assert!(disk.is_file(), "settings.json should exist after save");
    assert!(
        std::fs::read_to_string(&disk)
            .expect("read settings.json")
            .contains("\"selected_language\": \"en\""),
        "saved JSON should contain the persisted values"
    );

    let loaded = store.load().expect("reload settings");
    assert_eq!(loaded.selected_model, "ggml-small.bin");
    assert_eq!(
        loaded.custom_model_path.as_deref(),
        Some("/tmp/custom-whisper.gguf")
    );
    assert_eq!(loaded.selected_language, "en");
    assert!(loaded.translate_to_english);
    assert_eq!(loaded.custom_words, vec!["handy", "promptclear"]);
    assert_eq!(loaded.word_correction_threshold, 0.87);
    assert_eq!(loaded.app_language, "de");
    assert_eq!(
        loaded.custom_filler_words,
        Some(vec!["um".to_string(), "like".to_string()])
    );
    assert_eq!(
        loaded.model_unload_timeout,
        ModelUnloadTimeout::After10Minutes
    );
    assert_eq!(
        loaded.transcribe_accelerator,
        TranscribeAcceleratorSetting::Cpp
    );
    assert_eq!(loaded.ort_accelerator, OrtAcceleratorSetting::CoreMl);
    assert_eq!(loaded.transcribe_gpu_device, 1);
    assert_eq!(loaded.microphone_mode, MicrophoneMode::AlwaysOn);
    assert_eq!(loaded.selected_microphone.as_deref(), Some("Test Mic"));
    assert!(loaded.lazy_stream_close);
    assert_eq!(loaded.extra_recording_buffer_ms, 250);
    assert!(!loaded.streaming_enabled);
    assert!(loaded.spellcheck_enabled);
    assert_eq!(loaded.theme_mode, ThemeMode::Light);
    assert_eq!(loaded.accent_color, Accent::Teal);
    assert_eq!(loaded.corner_radius, CornerRadius::Rounded);

    // Fresh store instance reading the same file proves true re-load from disk.
    let reopened = JsonSettingsStore::new(disk.clone());
    let again = reopened.load().expect("reload from a fresh store");
    assert_eq!(again.selected_model, "ggml-small.bin");
    assert_eq!(
        again.transcribe_accelerator,
        TranscribeAcceleratorSetting::Cpp
    );

    std::fs::remove_dir_all(&dir).expect("clean up temp dir");
}

/* ────────────────────────────────────────────────────────────────────────── */
/* shared harness (tests 2-4)                                               */
/* ────────────────────────────────────────────────────────────────────────── */

struct Harness {
    _temp: tempfile::TempDir,
    settings_store: Arc<dyn SettingsStore>,
    model_manager: Arc<ModelManager>,
    transcription: Arc<TranscriptionManager>,
    events: Arc<Mutex<Vec<CoreEvent>>>,
}

/// One shared, lazily-built harness for the whole test process so tests 3 and
/// 4 reuse the same loaded model (with `--test-threads=1` they run in file
/// order and never race).
fn harness() -> Arc<Harness> {
    static HARNESS: OnceLock<Arc<Harness>> = OnceLock::new();
    Arc::clone(HARNESS.get_or_init(|| {
        let temp = tempfile::TempDir::new().expect("create harness temp dir");
        let settings_store: Arc<dyn SettingsStore> =
            Arc::new(JsonSettingsStore::new(temp.path().join("settings.json")));
        let events = Arc::new(Mutex::new(Vec::<CoreEvent>::new()));
        let sink: Arc<dyn EventSink> = {
            let events = events.clone();
            Arc::new(move |event: CoreEvent| events.lock().unwrap().push(event))
        };
        let model_manager = Arc::new(
            ModelManager::new(
                temp.path().join("models"),
                settings_store.clone(),
                sink.clone(),
            )
            .expect("ModelManager init"),
        );
        let transcription = Arc::new(
            TranscriptionManager::new(settings_store.clone(), sink, model_manager.clone())
                .expect("TranscriptionManager init"),
        );
        Arc::new(Harness {
            _temp: temp,
            settings_store,
            model_manager,
            transcription,
            events,
        })
    }))
}

/* ────────────────────────────────────────────────────────────────────────── */
/* test 2: VAD preload                                                       */
/* ────────────────────────────────────────────────────────────────────────── */

fn resolve_vad_path() -> Option<PathBuf> {
    for candidate in SILERO_VAD_CANDIDATES {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    let default_path = handy_core::paths::default_vad_model_path();
    if default_path.is_file() {
        return Some(default_path);
    }
    None
}

#[test]
fn vad_preload() {
    let Some(vad_path) = resolve_vad_path() else {
        eprintln!(
            "SKIP: vad_preload — silero_vad_v4.onnx not found at {:?}",
            SILERO_VAD_CANDIDATES
        );
        return;
    };
    let h = harness();
    let audio = AudioRecordingManager::new(
        h.settings_store.clone(),
        Arc::clone(&h.transcription.events),
        vad_path.clone(),
        h.transcription.stream_router(),
    );
    // preload_vad builds the recorder, which constructs SileroVad and loads
    // the ONNX through onnxruntime — a real VAD load, not a file existence
    // check. Ok(()) proves the model parses and initializes.
    audio
        .preload_vad()
        .unwrap_or_else(|e| panic!("preload_vad failed with real ONNX at {vad_path:?}: {e}"));
    println!("PASS: vad_preload — Silero VAD engine loaded from {vad_path:?}");
}

/* ────────────────────────────────────────────────────────────────────────── */
/* test 3: model load from disk (shared with test 4)                         */
/* ────────────────────────────────────────────────────────────────────────── */

const SEARCH_ROOTS: [&str; 2] = ["com.pais.handy/models", "PromptClear/models"];
const MAX_SEARCH_DEPTH: usize = 6;
const SPEECH_KEYWORDS: [&str; 16] = [
    "whisper",
    "ggml",
    "asr",
    "parakeet",
    "voxtral",
    "nemotron",
    "qwen3",
    "breeze",
    "canary",
    "gigaam",
    "moonshine",
    "sensevoice",
    "sense-voice",
    "cohere",
    "paraformer",
    "sherpa",
];

fn walk_gguf(dir: &Path, depth: usize, max_depth: usize, out: &mut Vec<PathBuf>) {
    if depth > max_depth {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if ft.is_dir() {
            walk_gguf(&path, depth + 1, max_depth, out);
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("gguf"))
        {
            out.push(path);
        }
    }
}

fn is_likely_speech_model(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    SPEECH_KEYWORDS.iter().any(|k| name.contains(k))
}

/// Find a real Whisper GGUF: the two Handy/PromptClear model dirs plus a
/// depth-limited walk of the whole `~/Library/Application Support` tree.
/// Prefers files whose names suggest a speech model; falls back to any GGUF.
fn find_gguf() -> Option<PathBuf> {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("/"));
    let mut candidates: Vec<PathBuf> = Vec::new();
    for sub in SEARCH_ROOTS {
        walk_gguf(&base.join(sub), 0, 2, &mut candidates);
    }
    walk_gguf(&base, 0, MAX_SEARCH_DEPTH, &mut candidates);
    candidates.sort();
    candidates.dedup();
    candidates
        .iter()
        .find(|p| is_likely_speech_model(p))
        .or_else(|| candidates.first())
        .cloned()
}

fn last_load_failure(events: &Mutex<Vec<CoreEvent>>) -> Option<String> {
    events
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find_map(|event| match event {
            CoreEvent::ModelStateChanged {
                event_type,
                error: Some(err),
                ..
            } if event_type == "loading_failed" => Some(err.clone()),
            _ => None,
        })
}

enum LoadOutcome {
    /// No Whisper GGUF exists on this machine — tests should SKIP.
    NoModel,
    /// Model loaded into the shared TranscriptionManager.
    Loaded(Arc<TranscriptionManager>, PathBuf, u64),
}

/// Register the found GGUF as the custom model, select it, load it in the
/// background, and wait (with a generous timeout) for it to come up. Panics
/// if a real model was found but fails to load or times out; returns NoModel
/// only when nothing was found. Shared by tests 3 and 4.
fn shared_load_outcome() -> &'static LoadOutcome {
    static OUTCOME: OnceLock<LoadOutcome> = OnceLock::new();
    OUTCOME.get_or_init(load_model_once)
}

fn load_model_once() -> LoadOutcome {
    let Some(path) = find_gguf() else {
        eprintln!("SKIP: no .gguf found under ~/Library/Application Support");
        return LoadOutcome::NoModel;
    };
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    println!(
        "INFO: found model candidate {} ({:.1} MiB); loading (timeout {:?})…",
        path.display(),
        size as f64 / (1024.0 * 1024.0),
        MODEL_LOAD_TIMEOUT
    );

    let h = harness();
    h.model_manager
        .register_custom_model(path.clone())
        .expect("register_custom_model");

    // initiate_model_load() reads `selected_model` from the persisted store.
    let mut settings = h.settings_store.load().expect("load settings");
    settings.selected_model = "custom-model".to_string();
    h.settings_store.save(&settings).expect("save settings");

    let load_started = Instant::now();
    h.transcription.initiate_model_load();

    let deadline = Instant::now() + MODEL_LOAD_TIMEOUT;
    loop {
        if h.transcription.is_model_loaded()
            && h.transcription.get_current_model().as_deref() == Some("custom-model")
        {
            break;
        }
        if let Some(msg) = last_load_failure(&h.events) {
            panic!(
                "model load failed after {:?}: {msg}",
                load_started.elapsed()
            );
        }
        if Instant::now() >= deadline {
            panic!(
                "timed out after {MODEL_LOAD_TIMEOUT:?} waiting for model load \
                 (loaded={}, current={:?})",
                h.transcription.is_model_loaded(),
                h.transcription.get_current_model(),
            );
        }
        std::thread::sleep(POLL_INTERVAL);
    }

    println!(
        "PASS: model loaded from {} ({} bytes, {:.1} MiB) in {:?}",
        path.display(),
        size,
        size as f64 / (1024.0 * 1024.0),
        load_started.elapsed()
    );
    LoadOutcome::Loaded(h.transcription.clone(), path, size)
}

#[test]
fn model_load_from_disk() {
    match shared_load_outcome() {
        LoadOutcome::NoModel => {
            eprintln!(
                "SKIP: model_load_from_disk — no Whisper GGUF found in \
                 ~/Library/Application Support (com.pais.handy/models, PromptClear/models, \
                 depth-limited glob)"
            );
        }
        LoadOutcome::Loaded(manager, path, size) => {
            assert!(manager.is_model_loaded(), "model must report loaded");
            assert_eq!(
                manager.get_current_model().as_deref(),
                Some("custom-model"),
                "loaded model id must be the registered custom model"
            );
            assert!(*size > 0, "model file must be non-empty");
            println!("model_load_from_disk: {path:?} ({size} bytes)");
        }
    }
}

/* ────────────────────────────────────────────────────────────────────────── */
/* test 4: transcribe a real wav                                             */
/* ────────────────────────────────────────────────────────────────────────── */

/// Depth-limited wav search over the Handy repo + common user locations,
/// skipping build artifacts.
fn walk_wav(dir: &Path, depth: usize, max_depth: usize, out: &mut Vec<PathBuf>) {
    if depth > max_depth {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if ft.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if ["target", "node_modules", ".git", "dist", "build", ".cache"]
                .iter()
                .any(|skip| name == *skip)
            {
                continue;
            }
            walk_wav(&path, depth + 1, max_depth, out);
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("wav"))
        {
            out.push(path);
        }
    }
}

/// Minimal WAV decode: hound (already a handy_core dependency) reads the
/// header + PCM16 samples; we downmix to mono and linearly resample to the
/// 16 kHz mono f32 the transcription engine expects.
fn decode_wav_16k_mono(path: &Path) -> Option<Vec<f32>> {
    let mut reader = hound::WavReader::open(path).ok()?;
    let spec = reader.spec();
    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.map(|v| v as f32 / i16::MAX as f32).unwrap_or(0.0))
        .collect();
    if samples.is_empty() {
        return Some(Vec::new());
    }
    let mono: Vec<f32> = if spec.channels <= 1 {
        samples
    } else {
        samples
            .chunks(spec.channels as usize)
            .map(|c| c.iter().sum::<f32>() / c.len() as f32)
            .collect()
    };
    if spec.sample_rate == WHISPER_SAMPLE_RATE {
        return Some(mono);
    }
    Some(linear_resample(
        &mono,
        spec.sample_rate,
        WHISPER_SAMPLE_RATE,
    ))
}

fn linear_resample(input: &[f32], in_rate: u32, out_rate: u32) -> Vec<f32> {
    let out_len = (input.len() as f64 * out_rate as f64 / in_rate as f64).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    let last = (input.len() - 1) as f64;
    for i in 0..out_len {
        let pos = if out_len <= 1 {
            0.0
        } else {
            i as f64 * last / (out_len - 1) as f64
        };
        let idx = pos.floor() as usize;
        let frac = pos - idx as f64;
        let a = input[idx];
        let b = if idx + 1 < input.len() {
            input[idx + 1]
        } else {
            a
        };
        out.push(a + (b - a) * frac as f32);
    }
    out
}

/// Fraction of the wav that the real Silero VAD classifies as speech
/// (480-sample / 30ms frames at 16 kHz).
fn speech_seconds(vad: &mut SileroVad, samples: &[f32]) -> f64 {
    const FRAME: usize = WHISPER_SAMPLE_RATE as usize * 30 / 1000; // 480
    let mut speech_frames = 0usize;
    for chunk in samples.chunks(FRAME) {
        if chunk.len() != FRAME {
            continue;
        }
        if let Ok(frame) = vad.push_frame(chunk) {
            if matches!(frame, handy_core::vad::VadFrame::Speech(_)) {
                speech_frames += 1;
            }
        }
    }
    speech_frames as f64 * 0.03
}

/// A *speech* wav: < 10 MB, and either confirmed by the real Silero VAD
/// (>= 0.5 s of speech) or, when the VAD onnx is missing, at least ~1 s long
/// (short UI sounds like Handy's 0.5 s pop/marimba jingles are rejected).
fn find_speech_wav() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    let mut collect = |dir: &Path, max_depth: usize| walk_wav(dir, 0, max_depth, &mut candidates);
    // Repo root = handy_core's manifest dir up two levels.
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    collect(&repo_root, 6);
    if let Some(home) = std::env::var_os("HOME") {
        for sub in ["Downloads", "Desktop", "Music"] {
            collect(&Path::new(&home).join(sub), 2);
        }
    }
    candidates.sort();
    candidates.dedup();

    let mut vad = resolve_vad_path().and_then(|p| SileroVad::new(p, 0.5).ok());

    let mut with_duration: Vec<(PathBuf, f64)> = candidates
        .into_iter()
        .filter_map(|path| {
            let Ok(meta) = std::fs::metadata(&path) else {
                return None;
            };
            if meta.len() >= 10 * 1024 * 1024 {
                return None;
            }
            let samples = decode_wav_16k_mono(&path)?;
            let duration = samples.len() as f64 / WHISPER_SAMPLE_RATE as f64;
            let is_speech = match &mut vad {
                Some(v) => speech_seconds(v, &samples) >= 0.5,
                None => duration >= 1.0,
            };
            is_speech.then_some((path, duration))
        })
        .collect();

    with_duration.sort_by(|a, b| b.1.total_cmp(&a.1));
    with_duration.first().map(|(p, _)| p.clone())
}

#[test]
fn transcribe_wav() {
    let (manager, model_path, model_size) = match shared_load_outcome() {
        LoadOutcome::NoModel => {
            eprintln!("SKIP: transcribe_wav — no Whisper GGUF available to transcribe with");
            return;
        }
        LoadOutcome::Loaded(tm, path, size) => (tm, path, size),
    };

    let Some(wav) = find_speech_wav() else {
        eprintln!(
            "SKIP: transcribe_wav — no speech wav (<10 MB, VAD- or duration-verified) \
             found in /Users/apple/code/Handy or ~/Downloads, ~/Desktop, ~/Music"
        );
        return;
    };
    let wav_size = std::fs::metadata(&wav).map(|m| m.len()).unwrap_or(0);
    println!(
        "INFO: transcribing {wav:?} ({wav_size} bytes) with {model_path:?} ({model_size} bytes)"
    );

    let samples = decode_wav_16k_mono(&wav).unwrap_or_else(|| panic!("failed to decode {wav:?}"));
    assert!(
        !samples.is_empty(),
        "decoded wav must contain samples: {wav:?}"
    );
    println!(
        "INFO: decoded {} f32 samples ({:.1} s @ {} Hz, mono)",
        samples.len(),
        samples.len() as f64 / WHISPER_SAMPLE_RATE as f64,
        WHISPER_SAMPLE_RATE
    );

    let text = manager
        .transcribe(samples)
        .unwrap_or_else(|e| panic!("transcribe failed for {wav:?}: {e}"));
    let trimmed = text.trim();
    assert!(
        !trimmed.is_empty(),
        "transcription produced empty text for {wav:?}"
    );
    let snippet: String = trimmed.chars().take(200).collect();
    println!(
        "PASS: transcription ({} chars): \"{snippet}\"",
        trimmed.chars().count()
    );
}
