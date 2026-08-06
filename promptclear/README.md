# PromptClear

**Offline voice notes with optional live spell & grammar check.**

PromptClear is a desktop note-taking app: type, or dictate, and get immediate
feedback on your writing. Built with plain Rust — eframe/egui — no Tauri or
webview. Fully offline at runtime (the only network use is optional model
downloads); it never sends your audio or text anywhere.

## What it is

- **Type or dictate** into a clean, distraction-free editor, with **optional
  live spelling + grammar** via `egui_spellcheck` (harper-core, English).
  Checking is opt-in and off by default (see [Spell checking](#spell-checking)).
- **Streaming dictation**: with a streaming-capable model and the
  `streaming_enabled` setting (default on), committed text streams into the
  editor while you speak, with tentative text shown under the editor; a
  post-processed final pass replaces the streamed text when you finish.
  Without a streaming-capable model (or with the setting off) it falls back
  to record → transcribe → append.
- **Global hotkey**: `Alt+Space` (rdev `Alt`/`AltGr` + `Space`) toggles
  dictation from any app, including while PromptClear sits in the background;
  on macOS this needs the Accessibility permission. Repaints are
  event-driven — an idle app runs no repaint timers, and mic-level events
  don't wake the UI. Without the permission the app-local shortcuts still
  work while focused: `Cmd+Enter` sends, `Cmd+Space` dictates, `Esc`
  cancels, `Cmd+,` toggles Settings.
- **Offline-first**: no account, no cloud, no telemetry.
- **Models**: locally run with Silero VAD silence filtering; pick one in-app
  (resumable downloads, SHA-256 verified) or point at any local GGUF/ONNX
  file (custom path).
- **Theming**: dark/light mode, 13 accent colors (including "gray mode"), and
  four corner-radius presets, applied instantly and persisted — see
  [Appearance & theming](#appearance--theming).
- **Status bar**: app state (Ready / Recording… / Transcribing… /
  Downloading…), a static `Spell: on/off` badge, per-frame spellcheck
  pass latency, word count, mic level meters while recording.
- **Low idle footprint by design**: no UI repaint timers when idle, the model
  unload watcher is a sleeping background thread, and the model loads lazily
  on first dictation (no numbers claimed here — measure them yourself).

## Architecture

- **`handy_core/`** — the core crate of the workspace; an audio/VAD/
  transcription core extracted from [Handy](https://github.com/cjpais/Handy)
  (MIT, (c) 2025 CJ Pais). No UI: cpal recording, Silero VAD, resampling,
  model management (download + custom local paths), Whisper/Parakeet-family
  transcription via transcribe-cpp (GGUF) and transcribe-rs (ONNX). Final
  and stable: 100 passing tests, no Tauri dependency.
- **`promptclear/`** — the eframe/egui app: editor, spell/grammar checking,
  status bar, hotkey handling, settings UI.
- **`egui-shadcn/`** — vendored widget library the UI is built on (see
  [Appearance & theming](#appearance--theming)).

`handy_core` decouples from the UI with two seams:
[`event::EventSink`](handy_core/src/event.rs) (a `Send + Sync` trait the UI
implements to receive `CoreEvent`s) and
[`settings::SettingsStore`](handy_core/src/settings.rs) (load/save of
`AppSettings`; a JSON file store is included). See
[API_SURFACE.md](API_SURFACE.md) for the full public API.

## Spell checking

Spell/grammar checking is **opt-in**: `spellcheck_enabled` (default **off**)
in Settings → Behavior. When off, the editor is a plain text area (and
nothing is loaded for checking); when on, `egui_spellcheck` (0.35) drives
both checks:

- **Spelling**: a bundled en-US Hunspell dictionary (~50k base entries, based
  on WordNet 2.1/SCOWL via JetBrains hunspell-dictionaries), compiled into
  the binary — no network needed.
- **Grammar**: harper-core rules (curated English dictionary), applied live.

Honest limitations:

- **No live issue count.** egui_spellcheck 0.35 exposes no public API for
  retrieving spelling/grammar hint counts (no `show_ew`/hint-count surface —
  only the widget and `suggestions()`), so the app cannot count issues per
  frame; the status bar shows a static `Spell: on/off` badge instead. The
  real indicators are the red underlines plus the per-frame latency readout.
- **Dictionaries can't be unloaded.** egui_spellcheck provides no way to
  fully unload the dictionaries/worker once loaded (crate limitation).
  Turning the setting off stops the checking, but the dictionaries stay
  resident (~a few MB) until the app quits.
- The dictionary isn't runtime-customizable beyond the "add word" personal
  dictionary. Domain vocabulary should go in the **custom words list in
  Settings** — for dictation it's applied through handy_core's
  `apply_custom_words` pipeline, rewriting a dictated term to the spelling
  you defined.

## Appearance & theming

Settings → Appearance:

- **Mode**: dark/light segmented control (`theme_mode`).
- **Accent**: a swatch picker row of 13 variants (`accent_color`) — Handy
  Pink (default) plus Neutral Gray ("gray mode") and 11 other colors.
- **Corner radius**: 4 presets (`corner_radius`) — 0 / 10 / 14 / 20 px —
  applied globally; the widget library exposes a single theme radius field.

All three apply instantly and persist in `settings.json`. The UI is built on
a **vendored copy of egui-shadcn** (MIT, Pavel Jankiewicz) at
`promptclear/egui-shadcn`, migrated from egui 0.33 → 0.35 (egui_flex 0.5 →
0.7); see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Toolchain

harper-core needs a **recent stable rustc**: it fails on 1.92.0 (HRTB
inference regression) and requires ~1.93+ — run `rustup update stable`; the
workspace's `rust-toolchain.toml` pins `channel = "stable"`. See BUILD.md.

## Build

Prerequisites: Rust (stable via rustup), a C/C++ toolchain with CMake, and
per-platform system packages — macOS Xcode CLT; Linux ALSA dev libs; Windows
MSVC Build Tools + Vulkan SDK for GPU builds. See [BUILD.md](BUILD.md).

```bash
cd promptclear
cargo build --release   # binary at target/release/
cargo run               # dev build for iteration
```

**First run**: the app starts with no model. Open **Settings → Model** and
either download one in-app or point the picker at any existing GGUF file —
e.g. one already downloaded by Handy (`~/Library/Application
Support/com.pais.handy/models`) or any Whisper GGUF from Hugging Face. The
VAD model is bundled at `promptclear/promptclear/resources/silero_vad_v4.onnx`
and auto-copied to the data dir on first launch.

## Data locations

Data dir: macOS `~/Library/Application Support/PromptClear`, Linux
`~/.local/share/PromptClear`, Windows `%APPDATA%\PromptClear`. Models live in
`<data dir>/models`, settings in `<data dir>/settings.json`. Set
`PROMPTCLEAR_DATA_DIR` to relocate the data dir; the bundled VAD model is
looked up in `PROMPTCLEAR_RESOURCES_DIR` first, then `<exe dir>/resources`,
then the crate's `resources/` folder.

## License & attribution

- `handy_core`: MIT, (c) 2025 CJ Pais — extracted from
  [Handy](https://github.com/cjpais/Handy). See
  [handy_core/LICENSE](handy_core/LICENSE).
- `egui_spellcheck`: MPL-2.0 · `harper-core`: Apache-2.0 · `spellbook`: MPL-2.0.
- `egui-shadcn`: MIT — vendored at `promptclear/egui-shadcn` (details in
  [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)).
- All third-party notices: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Measured baseline (macOS arm64, release build, no model loaded)

- Binary size: 42 MB (`strip = true`, LTO thin).
- Idle RSS with the window open and no model: ~113 MB (3 runs, min/median/max
  = 112.6 / 112.8 / 112.9 MiB) — dominated by egui/glow and the preloaded VAD
  model. The Whisper model is *not* resident until the first dictation, and
  the spelling/grammar dictionaries are only loaded once spellcheck is
  enabled (it is off by default).
- 1 of 3 startup runs in earlier testing exited with SIGTRAP right after
  microphone stream init (not reproduced; suspected audio-thread teardown
  flake) — not observed in the current baseline runs.

## Known limitations

- The spelling dictionary (~50k base entries) is smaller than commercial
  engines; expect more false positives on jargon until words are added to
  your personal dictionary.
- The global hotkey needs macOS Accessibility permission; on Wayland (Linux)
  global shortcuts are limited by the compositor (bind one yourself or use
  the app-local shortcuts).
- Grammar checking is English-only (harper-core has no non-English rule
  sets).