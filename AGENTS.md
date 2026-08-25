
```
bun install


bun run tauri dev
CMAKE_POLICY_VERSION_MINIMUM=3.5 bun run tauri dev

# Build for production
bun run tauri build

# Install DMG via CLI (no drag-and-drop needed) — mounts DMG, copies to /Applications, clears quarantine and launches:
hdiutil attach src-tauri/target/release/bundle/dmg/*.dmg -quiet && (pkill -x Handy || true) && sleep 1 && cp -R "/Volumes/Handy/Handy.app" /Applications/ && hdiutil detach "/Volumes/Handy" -quiet; xattr -dr com.apple.quarantine /Applications/Handy.app 2>/dev/null; open /Applications/Handy.app

# Frontend only development
bun run dev        # Start Vite dev server
bun run build      # Build frontend (TypeScript + Vite)
bun run preview    # Preview built frontend
```

**Linting and Formatting (run before committing):**

```bash
bun run lint              # ESLint for frontend
bun run lint:fix          # ESLint with auto-fix
bun run format            # Prettier + cargo fmt
bun run format:check      # Check formatting without changes
bun run format:frontend   # Prettier only
bun run format:backend    # cargo fmt only
```

**Model Setup (Required for Development):**

```bash
mkdir -p src-tauri/resources/models
curl -o src-tauri/resources/models/silero_vad_v4.onnx https://blob.handy.computer/silero_vad_v4.onnx
```

For detailed platform-specific build setup, see [BUILD.md](BUILD.md).

## Architecture Overview

### Backend Structure (src-tauri/src/)

- `lib.rs` - Main entry point, Tauri setup, manager initialization
- `managers/` - Core business logic:
  - `audio.rs` - Audio recording and device management
  - `model.rs` - Model downloading and management
  - `transcription.rs` - Speech-to-text processing pipeline
  - `history.rs` - Transcription history storage
  - `prompt_library.rs` - Prompt library (folders/tags/versions/FTS) — portable-aware via `portable::app_data_dir`
  - `prompt_history.rs` - Pasted-prompts log (now `rusqlite_migration`; legacy `prompt_history.db` kept for migration into `prompt_library.db`)
- `audio_toolkit/` - Low-level audio processing:
  - `audio/` - Device enumeration, recording, resampling
  - `vad/` - Voice Activity Detection (Silero VAD)
- `commands/` - Tauri command handlers for frontend communication
- `server.rs` - Loopback HTTP server (axum) for server mode — serves `dist/` without a WebView
- `overlay/` - Recording overlay façade (`mod.rs` + `webview.rs` + `native_macos.rs`/`native_windows.rs`; Linux stays WebView + `gtk_layer_shell`)
- `cli.rs` - CLI argument definitions (clap derive)
- `shortcut.rs` - Global keyboard shortcut handling
- `settings.rs` - Application settings management (schema version 2; `prompt_library_enabled`/`server_mode_enabled`/`server_port`/`server_bind`/`server_auth_token`/`overlay_native_enabled`)
- `signal_handle.rs` - `send_transcription_input()` reusable function
- `utils.rs` - Platform detection helpers

### Frontend Structure (src/)

- `App.tsx` - Main component with onboarding flow
- `components/` - React UI components:
  - `settings/` - Settings UI
  - `model-selector/` - Model management interface
  - `onboarding/` - First-run experience
  - `overlay/` - Recording overlay UI
  - `update-checker/` - App update notifications
  - `shared/`, `ui/`, `icons/`, `footer/` - Shared components
- `hooks/useSettings.ts` - Settings state management hook
- `stores/settingsStore.ts` - Zustand store for settings
- `bindings.ts` - Auto-generated Tauri type bindings (via tauri-specta)
- `overlay/` - Recording overlay window entry point
- `lib/types.ts` - Shared TypeScript type definitions

### Key Architecture Patterns

**Manager Pattern:** Core functionality organized into managers (Audio, Model, Transcription) initialized at startup and managed via Tauri state.

**Command-Event Architecture:** Frontend → Backend via Tauri commands; Backend → Frontend via events.

**Pipeline Processing:** Audio → VAD → Whisper/Parakeet → Text output → Clipboard/Paste

**State Flow:** Zustand → Tauri Command → Rust State → Persistence (tauri-plugin-store)

### Technology Stack

**Core Libraries:**

- `transcribe-cpp` - Local Whisper-family inference (GGML/GGUF) with GPU acceleration
- `transcribe-rs` - ONNX speech recognition (Parakeet, Moonshine, SenseVoice, etc.)
- `cpal` - Cross-platform audio I/O
- `vad-rs` - Voice Activity Detection
- `rdev` - Global keyboard shortcuts
- `rubato` - Audio resampling
- `rodio` - Audio playback for feedback sounds

### Application Flow

1. **Initialization:** App starts minimized to tray, loads settings, initializes managers
2. **Model Setup:** First-run downloads preferred Whisper model (Small/Medium/Turbo/Large)
3. **Recording:** Global shortcut triggers audio recording with VAD filtering
4. **Processing:** Audio sent to Whisper model for transcription
5. **Output:** Text pasted to active application via system clipboard

### Settings System

Settings are stored using Tauri's store plugin with reactive updates:

- Keyboard shortcuts (configurable, supports push-to-talk)
- Audio devices (microphone/output selection)
- Model preferences (Small/Medium/Turbo/Large Whisper variants)
- Audio feedback and translation options

### Single Instance Architecture

The app enforces single instance behavior — launching when already running brings the settings window to front rather than creating a new process. Remote control flags (`--toggle-transcription`, etc.) work by launching a second instance that sends args to the running instance via `tauri_plugin_single_instance`, then exits.

## Internationalization (i18n)

All user-facing strings must use i18next translations. ESLint enforces this (no hardcoded strings in JSX).

**Adding new text:**

1. Add key to `src/i18n/locales/en/translation.json`
2. Use in component: `const { t } = useTranslation(); t('key.path')`

**File structure:**

```
src/i18n/
├── index.ts           # i18n setup
├── languages.ts       # Language metadata
└── locales/
    ├── en/translation.json  # English (source)
    ├── de/, es/, fr/, ja/, ru/, zh/, ...
    └── ...
```

For translation contribution guidelines, see [CONTRIBUTING_TRANSLATIONS.md](CONTRIBUTING_TRANSLATIONS.md).

## Code Style

**Rust:**

- Run `cargo fmt` and `cargo clippy` before committing
- Handle errors explicitly (avoid unwrap in production)
- Use descriptive names, add doc comments for public APIs

**TypeScript/React:**

- Strict TypeScript, avoid `any` types
- Functional components with hooks
- Tailwind CSS for styling
- Path aliases: `@/` → `./src/`

## CLI Parameters

Handy supports command-line parameters on all platforms for integration with scripts, window managers, and autostart configurations.

**Implementation:** `cli.rs` (definitions), `main.rs` (parsing), `lib.rs` (applying), `signal_handle.rs` (shared logic)

| Flag                     | Description                                                |
| ------------------------ | ---------------------------------------------------------- |
| `--toggle-transcription` | Toggle recording on/off on a running instance              |
| `--toggle-post-process`  | Toggle recording with post-processing on/off               |
| `--cancel`               | Cancel the current operation on a running instance         |
| `--start-hidden`         | Launch without showing the main window (tray icon visible) |
| `--no-tray`              | Launch without system tray (closing window quits the app)  |
| `--debug`                | Enable debug mode with verbose (Trace) logging             |

> **Server mode is settings-only:** `server_mode_enabled`/`server_port`/`server_auth_token` are toggled in Settings → Advanced → Server Mode (requires restart). There is **no** `--server-port` CLI — the port is read from `settings_store.json` (portable-aware via `portable::store_path`) and the actual bound port is kept in `ServerState.port` (ephemeral, tries `17373..17383`). Plain second-instance launch with `server_mode_enabled` and no `main` WebView forwards to `opener.open_url(http://127.0.0.1:port)` instead of `show_main_window` (`lib.rs:841` `single_instance` branch).

**Key design decisions:**

- CLI flags are runtime-only overrides — they do NOT modify persisted settings (including `server_port`)
- Remote control flags work via `tauri_plugin_single_instance`: second instance sends args, then exits
- `send_transcription_input()` in `signal_handle.rs` is shared between signal handlers and CLI

## Debug Mode

Access debug features: `Cmd+Shift+D` (macOS) or `Ctrl+Shift+D` (Windows/Linux)

## Platform Notes

- **macOS**: Metal acceleration, accessibility permissions required for keyboard shortcuts. Native overlay (`overlay/native_macos.rs` + Swift `native-overlay/macos/`) requires **Xcode 15+** (not just CLT) for `tauri-nspanel 2.1` — `build.rs` links `liboverlay_macos.a` via `swiftc`/`libtool`; `is_command_line_tools_only` fallback keeps a stub overlay when Xcode is absent.
- **Windows**: Vulkan acceleration, code signing. Native overlay uses `softbuffer 0.4` + `tiny-skia 0.11` (CPU raster, `HWND` layered window `WS_EX_LAYERED|TOPMOST`) — **not** `wgpu`/GPU (would conflict with Vulkan backend).
- **Linux**: OpenBLAS + Vulkan, limited Wayland support, overlay uses GTK layer shell (`gtk_layer_shell::is_supported:121`, `update_gtk_layer_shell_anchors:77`). Disable with `HANDY_NO_GTK_LAYER_SHELL=1`. Native overlay flag defaults `false` on Linux and is gated at runtime (`overlay/mod.rs:174` `is_native_enabled`).

## Troubleshooting
See the [Troubleshooting](README.md#troubleshooting) section in README.md.

## GitHub workflow for AI coding assistants
**Commits:** Use conventional commit prefixes (`feat:`, `fix:`, `docs:`, `refactor:`, `chore:`). Focus the message on _why_, not _what_.
