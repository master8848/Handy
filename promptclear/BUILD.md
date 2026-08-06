# Building PromptClear

PromptClear is a plain Cargo workspace (`handy_core` + `promptclear` app crate,
eframe/egui). It needs a Rust toolchain, a C/C++ compiler + CMake (for
`transcribe-cpp` and the ONNX Runtime native pieces), and platform system
packages.

## Common to all platforms

```bash
# 1. Install Rust stable (https://rustup.rs) if you don't have it
rustup default stable
rustup update stable        # see "Toolchain" below — older stable rustc won't compile harper-core

# 2. Build / run from the workspace root
cd promptclear
cargo build --release
cargo run              # dev build
```

### Toolchain

`harper-core` (the grammar engine, pulled in by egui_spellcheck) needs a
**recent stable rustc**: it fails to compile on 1.92.0 due to an HRTB inference
regression and requires roughly 1.93+. If you hit a confusing borrow-checker /
trait-resolution error inside a harper crate, update Rust first:
`rustup update stable`. The workspace pins `channel = "stable"` in
`rust-toolchain.toml` (no specific version), so rustup will pick up the latest
stable automatically.

The first build downloads and compiles many dependencies (including the native
`transcribe-cpp` and ONNX Runtime builds) and can take a long time; subsequent
builds are incremental.

## macOS

- **Xcode Command Line Tools**: `xcode-select --install`.
- **CMake** on PATH: `brew install cmake`.
- The default build uses the **Metal** backend for transcribe-cpp (Apple
  Silicon and Intel both fine). The Metal shader compilation needs `xcrun`
  (from the CLT).
- First launch will prompt for **microphone** permission (TCC). If you deny it,
  grant it later in System Settings → Privacy & Security → Microphone and
  restart. If recording silently captures nothing, that is the first thing to
  check.
- The global hotkey (`Alt+Space`) additionally requires the **Accessibility**
  permission for the rdev listener: System Settings → Privacy & Security →
  Accessibility. Without it, app-local shortcuts (`Cmd+Enter`, `Cmd+Space`,
  `Esc`, `Cmd+,`) still work while the window is focused.

## Windows

- **Visual Studio Build Tools** (or VS) with the "Desktop development with
  C++" workload — MSVC `cl.exe` is required.
- **CMake** on PATH: `winget install Kitware.CMake`.
- x64 builds use the `dynamic-backends` transcribe-cpp profile (loadable ggml
  backends + dynamically linked ONNX Runtime), which needs the **Vulkan SDK**
  from LunarG (`winget install KhronosGroup.VulkanSDK`) — ggml's Vulkan shader
  generation needs the SDK's headers and tools. Windows on ARM builds are
  CPU-only and skip it.
- Long path issues: if the native build fails deep in the CMake tree with
  path-length errors, enable long paths in the registry
  (`LongPathsEnabled=1`) — or check out the repo at a shallower path.
- If `onnxruntime.dll` is not found at runtime, the app expects it beside the
  exe (or on `PATH`) for the dynamic x64 build.

## Linux

- **ALSA dev libs** are required for cpal:
  - Ubuntu/Debian: `sudo apt install build-essential cmake libasound2-dev pkg-config`
  - Fedora: `sudo dnf install alsa-lib-devel pkgconf cmake gcc-c++`
  - Arch: `sudo pacman -S base-devel alsa-lib cmake`
- The Vulkan backend for transcribe-cpp also needs a Vulkan SDK
  (`libvulkan-dev` + `glslc`/`glslang-tools` on Debian/Ubuntu, or the distro's
  `vulkan-devel` equivalents) and a `mesa-vulkan-drivers` runtime if you want
  GPU acceleration; a CPU-only fallback works without it.
- **PipeWire/PulseAudio/ALSA**: cpal records through whatever ALSA exposes.
  If the default device fails, set the microphone explicitly in Settings, and
  make sure `pipewire-pulse`/`pulseaudio` is running if your distro uses them.
- Headless CI boxes have no audio devices; unit tests that don't open the mic
  run fine, but dictation needs a real device.
- Wayland: global shortcut registration is limited by the compositor — bind a
  compositor-level shortcut instead (see README "Known limitations").

## Model setup

- **In-app download**: open Settings → Model, pick a model, and press
  Download. Downloads resume after interruption and are SHA-256 verified.
- **Custom GGUF/ONNX path**: Settings → Model → "Choose custom GGUF/ONNX…",
  pick any `.gguf`/`.bin`/`.onnx` file on disk. It is never downloaded or
  copied — the app reads it in place from `custom_model_path`.
- **VAD model**: `silero_vad_v4.onnx` is bundled at
  `promptclear/promptclear/resources/silero_vad_v4.onnx` and auto-copied to
  `<data dir>/models/` on first launch (see "Failed to create SileroVad"
  below if that copy goes missing).

## Troubleshooting

### CMake errors

If the `transcribe-cpp-sys`/ggml CMake configure fails with an error about
`cmake_policy` version (a "policy version ... not recognized" or similar), the
known workaround is:

```bash
CMAKE_POLICY_VERSION_MINIMUM=3.5 cargo build --release
```

This raises the minimum policy version and is harmless for these pinned
builds. Prefer it over editing lockfiles.

### ONNX Runtime downloads at build time

The ONNX parts (`transcribe-rs` with its `onnx` feature → the `ort` crate)
download prebuilt ONNX Runtime binaries during their build scripts. This needs
network access **once**; the artifacts are cached in the cargo cache
(`~/.cargo`) and reused afterwards. In offline/airgapped environments, prime
the cache first, or expect the build to fail at that step. If a download
appears stuck or fails midway, delete the half-fetched cache entry and retry.

### "Failed to create SileroVad" / missing VAD model

PromptClear bundles the VAD model at
`promptclear/promptclear/resources/silero_vad_v4.onnx` and copies it into the
data dir on first launch. If that copy is missing or truncated, the app errors
when starting a recording. Fix: place a valid `silero_vad_v4.onnx` in the data
dir's `models/` folder (macOS: `~/Library/Application Support/PromptClear/
models/`), or set `PROMPTCLEAR_DATA_DIR` to a data dir that contains it. The
bundled-copy lookup honors `PROMPTCLEAR_RESOURCES_DIR` before falling back to
`<exe dir>/resources` and the crate's `resources/`.

### macOS: microphone permission (TCC)

- Symptom: recording starts but produces no/empty transcription, or
  `PermissionDenied` errors in the log.
- Fix: System Settings → Privacy & Security → Microphone → enable PromptClear,
  then restart the app. The core detects this and surfaces
  `is_microphone_access_denied`.

### macOS: global hotkey does nothing

- Symptom: `Alt+Space` is dead outside the app window, but in-app shortcuts
  work.
- Fix: System Settings → Privacy & Security → Accessibility → enable
  PromptClear. The rdev global listener needs it; macOS will not prompt, so
  grant it manually and restart.

### Linux: no audio device / ALSA errors

- `cpal` needs ALSA dev libs at build time and a working ALSA/PipeWire runtime
  at run time.
- Check `aplay -l` (device present?) and `systemctl --user status pipewire`.
- If a specific device misbehaves, pick it explicitly in Settings rather than
  the system default.

### General

- `cargo run --release` vs `cargo run`: use the release build for any
  performance judgement — debug builds are dramatically slower for
  transcription and spell checking.
- Watch startup logs with `RUST_LOG=debug` (or `trace`) for model/VAD path
  resolution issues.
- If the build fails inside a harper-* crate with a trait/borrow error on
  rustc ≤ 1.92, see "Toolchain" above — update stable first.
