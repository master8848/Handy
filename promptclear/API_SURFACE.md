# handy_core — public API surface

Documented from `handy_core/src/` as it exists in this workspace. The crate is
**final**: it builds standalone with **no Tauri dependency**, decouples from UI
frameworks via the `event::EventSink` and `settings::SettingsStore` seams, and
passes its full test suite (100 tests). Earlier revisions of this document
listed `tauri::AppHandle` constructor parameters and "extraction in progress"
caveats — those are gone and must not be re-added.

## Modules (`lib.rs`)

```
audio, audio_manager, catalog, constants, download, event, gguf_meta,
model, model_capabilities, paths, settings, text, transcription, utils, vad
```

## Re-exported at crate root (`handy_core::*`)

| Item | Kind | Notes |
| --- | --- | --- |
| `AudioRecorder` | struct | cpal recorder, VAD + resample pipeline |
| `CpalDeviceInfo` | struct | `{ index: String, name, is_default, device }` |
| `VadPolicy` | enum | `Disabled \| Offline \| Streaming` |
| `AudioRecordingManager` | struct | high-level record/mic lifecycle |
| `CoreEvent`, `EventSink` | enum / trait | see below |
| `DownloadProgress`, `EngineType`, `ModelInfo`, `ModelManager`, `ModelSource` | structs/enums | model registry + downloads |
| `TranscriptionManager` | struct | batch + streaming transcription |
| `AppSettings`, `SettingsStore`, `JsonSettingsStore` | struct/trait/struct | settings |
| `SileroVad`, `VadFrame`, `VoiceActivityDetector` | struct/enum/trait | VAD |
| `apply_custom_words`, `filter_transcription_output` | fns | text post-processing |
| `is_windows_x64_emulated_on_arm64` | fn | platform helper |
| `get_cpal_host`, `is_microphone_access_denied`, `is_no_input_device_error`, `list_input_devices`, `list_output_devices`, `read_wav_samples`, `save_wav_file`, `verify_wav_file` | fns | audio helpers |

Also public but **not** re-exported at the root — reachable via their module:

- `handy_core::audio::FrameResampler` — `new(in_hz: usize, out_hz: usize, frame_dur: Duration)`; `push(&mut self, src: &[f32], emit: impl FnMut(&[f32]))` / `finish(&mut self, emit: impl FnMut(&[f32]))` / `reset()`
- `handy_core::audio::AudioVisualiser` — `new(sample_rate: u32, window_size: usize, buckets: usize, freq_min: f32, freq_max: f32)`; `feed(&mut self, samples: &[f32]) -> Option<Vec<f32>>` (16 buckets, 0.0–1.0); `reset()`
- `handy_core::vad::{SmoothedVad, VAD_PREFILL_FRAMES (15), VAD_OFFLINE_HANGOVER_FRAMES (15), VAD_STREAMING_HANGOVER_FRAMES (55), VAD_ONSET_FRAMES (2)}`
- `handy_core::constants::WHISPER_SAMPLE_RATE` (`u32` = 16000)
- `handy_core::catalog::CATALOG` — bundled model catalog descriptors (`Lazy<Vec<ModelDescriptor>>`); helpers `mirror_fallbacks(model_id)`, `file_in_catalog(filename, repo_id)`, `rank_of(model_id)`; `MirrorFile` struct
- `handy_core::model_capabilities::{CapabilityProbe, GgufHeaderProber, Compatibility}` — GGUF header capability probing
- `handy_core::gguf_meta::{GgufMetadata, GgufValue, parse_header}` — parsed GGUF metadata
- `handy_core::model::{effective_language, ModelDescriptor, QuantFile, DiskStatus}` — language coercion + catalog descriptors
- `handy_core::paths` — data-dir helpers (see below)

The `download` module is public but exposes **no** public items — resumable
downloads are internal to `ModelManager`.

## `CoreEvent` (`event.rs`)

All variants are `Debug, Clone, Serialize, Deserialize` and mirror the event
names Handy emits through Tauri.

| Variant | Meaning |
| --- | --- |
| `ModelsUpdated` | The model registry changed (list should refresh). |
| `ModelDeleted { model_id: String }` | A model was removed from disk/registry. |
| `ModelDownloadProgress { model_id: String, downloaded: u64, total: u64, percentage: f64 }` | Resumable download progress. |
| `ModelDownloadComplete { model_id: String }` | Download finished successfully. |
| `ModelDownloadCancelled { model_id: String }` | Download cancelled; partial file kept for resume. |
| `ModelExtractionStarted { model_id: String }` | Extracting an archived model (tar.gz). |
| `ModelExtractionComplete { model_id: String }` | Extraction finished. |
| `ModelExtractionFailed { model_id: String, error: Option<String> }` | Extraction failed. |
| `ModelVerificationStarted { model_id: String }` | SHA-256 verification of a download began. |
| `ModelVerificationComplete { model_id: String }` | Verification passed. |
| `ModelVerificationFailed { model_id: String, error: Option<String> }` | Verification failed; the partial file was removed. |
| `ModelStateChanged { event_type: String, model_id: Option<String>, model_name: Option<String>, error: Option<String> }` | Generic model-state transition ("model-state-changed"). |
| `StreamText { committed: String, tentative: String }` | Streaming transcription: stable prefix + volatile suffix. |
| `StreamPhase { phase: String }` | Streaming phase string ("listening", "processing", "working", …). |
| `AudioLevels(Vec<f32>)` | Microphone level buckets (16 values, 0.0–1.0) for visualization. |

## `EventSink` trait (`event.rs`)

```rust
pub trait EventSink: Send + Sync {
    fn on_event(&self, event: CoreEvent);
}
```

Blanket impl: any `F: Fn(CoreEvent) + Send + Sync` is an `EventSink`. The UI
wires this to its own channel.

## `SettingsStore` trait + `JsonSettingsStore` (`settings.rs`)

```rust
pub trait SettingsStore: Send + Sync {
    fn load(&self) -> Result<AppSettings>;
    fn save(&self, settings: &AppSettings) -> Result<()>;
}

pub struct JsonSettingsStore;                    // pretty-printed JSON at a caller-supplied path
impl JsonSettingsStore { pub fn new(path: PathBuf) -> Self; }
```

Missing file → `AppSettings::default()`. Free shims (API-compatible with
Handy's `get_settings`/`write_settings`) and the unload-timeout helper:

```rust
pub fn get_settings(store: &dyn SettingsStore) -> AppSettings;   // default + warn on load failure
pub fn write_settings(store: &dyn SettingsStore, settings: &AppSettings) -> Result<()>;
pub fn model_unload_timeout_seconds(timeout: ModelUnloadTimeout) -> Option<u64>;
// After5Minutes → 300, After10Minutes → 600, After30Minutes → 1800, Never → None
```

## `AppSettings` fields with defaults (`settings.rs`)

All fields `#[serde(default)]`; the struct derives `Default`, so the defaults
below are the Rust defaults (the UI may set different effective values).

| Field | Type | Default |
| --- | --- | --- |
| `selected_model` | `String` | `""` (e.g. `"small"`) |
| `custom_model_path` | `Option<String>` | `None` — absolute path to a local GGUF/ONNX file (PromptClear extension) |
| `selected_language` | `String` | `""` |
| `translate_to_english` | `bool` | `false` |
| `custom_words` | `Vec<String>` | `[]` |
| `word_correction_threshold` | `f64` | `0.0` |
| `app_language` | `String` | `""` |
| `custom_filler_words` | `Option<Vec<String>>` | `None` (`Some(vec![])` disables filtering) |
| `model_unload_timeout` | `ModelUnloadTimeout` | `Never` |
| `transcribe_accelerator` | `TranscribeAcceleratorSetting` | `Auto` |
| `ort_accelerator` | `OrtAcceleratorSetting` | `Cpu` |
| `transcribe_gpu_device` | `i32` | `0` |
| `microphone_mode` | `MicrophoneMode` | `OnDemand` |
| `selected_microphone` | `Option<String>` | `None` (system default) |
| `lazy_stream_close` | `bool` | `false` — keep the mic stream open ~30 s after an on-demand recording (PromptClear extension) |
| `extra_recording_buffer_ms` | `u64` | `0` — keep capturing this long after stop is pressed (PromptClear extension) |
| `streaming_enabled` | `bool` | `true` — live streaming dictation (PromptClear extension) |
| `spellcheck_enabled` | `bool` | `false` — opt-in editor spell/grammar checking (PromptClear extension) |
| `theme_mode` | `ThemeMode` | `Dark` |
| `accent_color` | `Accent` | `HandyPink` |
| `corner_radius` | `CornerRadius` | `Default` |

Supporting enums (all `Serialize`/`Deserialize`; serde names as given):

- `TranscribeAcceleratorSetting`: `Ort` ("ort") · `Cpp` ("cpp") · `Auto` ("auto", default)
- `OrtAcceleratorSetting`: `Cpu` ("cpu", default) · `CpuMemOptimize` ("cpu-mem-optimize") · `Cuda` ("cuda") · `CudaTensorrt` ("cuda-tensorrt") · `DirectMl` ("directml") · `CoreMl` ("coreml") · `Auto` ("auto")
- `ModelUnloadTimeout`: `After5Minutes` ("5min") · `After10Minutes` ("10min") · `After30Minutes` ("30min") · `Never` ("never", default)
- `MicrophoneMode`: `OnDemand` ("ondemand", default) · `AlwaysOn` ("alwayson")
- `ThemeMode`: `Dark` ("dark", default) · `Light` ("light")
- `Accent` (13 variants, snake_case serde): `HandyPink` ("handy_pink", default) · `NeutralGray` ("neutral_gray", the monochrome "gray mode") · `Blue` · `Sky` · `Cyan` · `Teal` · `Green` · `Lime` · `Amber` · `Orange` · `Red` · `Violet` · `Purple`
- `CornerRadius` (4 presets): `Sharp` ("sharp", 0 px) · `Default` ("default", 10 px, default) · `Rounded` ("rounded", 14 px) · `VeryRounded` ("very_rounded", 20 px)

## Managers — constructor signatures as in the source

No manager takes a Tauri handle anymore.

| Type | Signature |
| --- | --- |
| `AudioRecorder` | `pub fn new() -> Result<Self, Box<dyn std::error::Error>>`; builders `with_vad(detector: Box<dyn VoiceActivityDetector>, offline_hangover_frames: usize, streaming_hangover_frames: usize)`, `with_level_callback<F: Fn(Vec<f32>) + Send + Sync + 'static>`, `with_audio_callback<F: Fn(&[f32]) + Send + Sync + 'static>`; `open(device: Option<Device>)`, `start(vad_policy: VadPolicy)`, `stop() -> Result<Vec<f32>, Box<dyn Error>>`, `close()`, `is_capture_worker_dead()`. |
| `AudioRecordingManager` | `pub fn new(settings_store: Arc<dyn SettingsStore>, events: Arc<dyn EventSink>, vad_model_path: PathBuf, stream_router: Arc<StreamRouter>) -> Self`. Opens the mic immediately when `microphone_mode == AlwaysOn`. |
| `ModelManager` | `pub fn new(models_dir: PathBuf, settings_store: Arc<dyn SettingsStore>, events: Arc<dyn EventSink>) -> Result<Self>`. Seeds the bundled catalog, discovers custom `.bin`/`.gguf` files in `models_dir`, discovers GGUF models in the shared HF cache, migrates bundled/GigaAM layouts, and auto-selects a downloaded model if none is selected. |
| `TranscriptionManager` | `pub fn new(settings_store: Arc<dyn SettingsStore>, events: Arc<dyn EventSink>, model_manager: Arc<ModelManager>) -> Result<Self>`. Starts the idle-unload watcher thread. |
| `SileroVad` | `pub fn new<P: AsRef<Path>>(model_path: P, threshold: f32) -> Result<Self>` (30 ms frames at 16 kHz; threshold must be 0.0–1.0). |
| `SmoothedVad` | `pub fn new(inner_vad: Box<dyn VoiceActivityDetector>, prefill_frames: usize, hangover_frames: usize, onset_frames: usize) -> Self` |

`StreamRouter` (`transcription.rs`) is the frame channel shared between the
recorder and the streaming worker: `pub fn feed(&self, frame: &[f32])`,
`pub fn is_open(&self) -> bool`. Its constructor is **private** — the only way
to get one is `TranscriptionManager::stream_router()` (the recorder receives an
`Arc<StreamRouter>` from the UI at construction).

## `VoiceActivityDetector` trait (`vad/mod.rs`)

```rust
pub trait VoiceActivityDetector: Send + Sync {
    fn push_frame<'a>(&'a mut self, frame: &'a [f32]) -> Result<VadFrame<'a>>;
    fn is_voice(&mut self, frame: &[f32]) -> Result<bool>;  // default: via push_frame
    fn set_hangover_frames(&mut self, _frames: usize) {}    // optional
    fn reset(&mut self) {}                                  // optional
}
```

`VadFrame<'a>`: `Speech(&'a [f32])` (may aggregate prefill + current +
hangover frames) or `Noise`, with `is_speech()`.

## `AudioRecordingManager` methods (`audio_manager.rs`)

`new`, `preload_vad() -> Result<(), anyhow::Error>`,
`start_microphone_stream() -> Result<(), anyhow::Error>`,
`stop_microphone_stream()`, `update_mode(new_mode: MicrophoneMode) -> Result<(), anyhow::Error>`,
`update_selected_device() -> Result<(), anyhow::Error>`, `invalidate_device_cache()`,
`try_start_recording(binding_id: &str, vad_policy: VadPolicy) -> Result<(), String>`,
`stop_recording(binding_id: &str, cancel_generation: u64) -> Option<Vec<f32>>`
(extra `extra_recording_buffer_ms` tail is applied here; pads sub-second audio),
`cancel_recording()`, `is_recording() -> bool` (lock-free atomic mirror),
`cancel_generation() -> u64`, `was_cancelled_since(generation: u64) -> bool`.

`RecordingState` (`pub enum`): `Idle` · `Recording { binding_id: String }` ·
`Stopping`.

The old Handy `apply_mute`/`remove_mute` methods are **gone** in this crate.

## `ModelManager` methods (`model.rs`)

`new`, `get_available_models() -> Vec<ModelInfo>` (catalog-rank sorted),
`get_model_info(model_id: &str) -> Option<ModelInfo>`,
`rescan_local_models() -> Result<()>` (single-flight disk re-scan),
`set_runtime_capabilities(model_id, supports_streaming, supports_translation, supports_language_detection, supported_languages: Vec<String>)`,
`download_model(model_id: &str) -> Result<()>` (async), `download_model_sync(model_id: &str) -> Result<()>`
(sync wrapper that runs the download on a short-lived tokio runtime),
`delete_model(model_id: &str) -> Result<()>`,
`get_model_path(model_id: &str) -> Result<PathBuf>`,
`register_custom_model(path: PathBuf) -> Result<()>` (registers as id `"custom-model"`,
persists `custom_model_path` in settings),
`unregister_custom_model()` (clears `custom_model_path`),
`cancel_download(model_id: &str) -> Result<()>`.

Downloads support **resume**: partial files are kept and continued via
`Content-Range`, with a 60 s stall watchdog and SHA-256 verification
(`download.rs`). Sources: direct URL (`blob.handy.computer`-style), Hugging
Face Hub via hf-hub into the shared HF cache (with retry + mirror fallback),
or local disk. Engine routing:
`EngineType` = `TranscribeCpp | Parakeet | Moonshine | MoonshineStreaming |
SenseVoice | GigaAM | Canary | Cohere` (`TranscribeCpp` covers the whole
GGML/GGUF family — architecture is auto-detected from the file).
`ModelSource` = `Url { url, sha256: Option<String> } | HuggingFace { repo_id, revision } | Local | LocalPath`
(`LocalPath`, serde `"local-path"`, is the PromptClear custom-model extension —
the path lives in `AppSettings::custom_model_path` and is never downloaded).

Free function: `effective_language(intent: &str, supported_languages: &[String], supports_language_detection: bool) -> String`
— capability-aware coercion of the persisted language intent, never written
back to settings.

`ModelInfo` (`pub struct`, all fields `pub`): `id, name, description, filename,
source, size_mb, is_downloaded, is_downloading, partial_size, is_directory,
engine_type, accuracy_score (0.0–1.0), speed_score (0.0–1.0),
supports_translation, is_recommended, supported_languages, supports_language_selection,
is_custom, supports_streaming, supports_language_detection`.

`DownloadProgress { model_id: String, downloaded: u64, total: u64, percentage: f64 }`.

Also public in the module: `ModelDescriptor` (catalog spec), `QuantFile`
(`{ filename, quant, size_bytes, sha256: Option<String> }`), `DiskStatus`
(`{ is_downloaded, is_downloading, partial_size }`).

## `TranscriptionManager` methods (`transcription.rs`)

`new`, `is_model_loaded() -> bool`, `reload_model_on_next_use()`,
`try_start_loading() -> Option<LoadingGuard>` (RAII guard, `pub struct`),
`unload_model() -> Result<()>`, `maybe_unload_immediately(context: &str)`
(no-op debug-log in this crate — the idle watcher handles unloads),
`load_model(model_id)`, `load_model_with_device(model_id, device_index: Option<usize>)`,
`initiate_model_load()` (background load of the selected model),
`get_current_model() -> Option<String>`, `current_backend() -> Option<String>`,
`is_streaming() -> bool`, `stream_router() -> Arc<StreamRouter>`,
`start_stream()`, `finalize_stream() -> Result<Option<String>>`
(`Ok(None)` → caller may fall back to batch), `cancel_stream()`,
`emit_stream_working(kind: StreamWorkKind)`,
`transcribe(audio: Vec<f32>) -> Result<String>` (batch).

Free functions in the module: `init_transcribe_backend()`,
`describe_compute_devices() -> Vec<String>`, `get_available_accelerators() -> AvailableAccelerators`,
`apply_accelerator_settings(store: &dyn SettingsStore)` (**no Tauri handle** —
applies the ORT accelerator to the transcribe-rs global).

Public event/state structs: `ModelStateEvent { event_type, model_id, model_name, error }`
(Serialize only), `StreamTextEvent { committed: String, tentative: String }`
(Ser+De), `StreamPhaseEvent { phase: StreamPhase, kind: Option<StreamWorkKind> }`
(Ser+De, `kind` skipped when `None`), `StreamPhase` (`Listening`/`Working`,
lowercase serde), `StreamWorkKind` (`Transcribing`/`Polishing`, lowercase
serde), `GpuDeviceOption { id: i32, name: String, total_vram_mb: usize }`,
`AvailableAccelerators { transcribe: Vec<String>, ort: Vec<String>, gpu_devices: Vec<GpuDeviceOption> }`.

## Text post-processing (`text.rs`)

- `apply_custom_words(text: &str, custom_words: &[String], threshold: f64) -> String` —
  fuzzy custom-word correction: Levenshtein + Soundex + n-gram matching (1–3 words),
  case-preserving; ASCII-only for the fuzzy path.
- `filter_transcription_output(text: &str, lang: &str, custom_filler_words: &Option<Vec<String>>) -> String` —
  removes filler words (language-aware defaults; custom list overrides), collapses
  3+ word stutters, trims whitespace.

## Paths (`paths.rs`)

`data_dir_override()` (reads `PROMPTCLEAR_DATA_DIR`), `default_data_dir()`
(`<dirs::data_dir()>/PromptClear`), `default_models_dir()`,
`default_settings_path()`, `default_vad_model_path()`
(`<data dir>/models/silero_vad_v4.onnx`).

## Compatibility

`handy_core` has **no** `tauri` dependency anywhere in its sources; every
manager constructor takes `(settings_store, events[, …])` as shown above. The
`portable`/`helpers` modules that earlier revisions referenced do not exist and
are not needed. Anything claiming otherwise is stale.
