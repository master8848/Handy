# Handy CLI Reference

Handy's CLI is built with **[clap 4 (derive)]** (`src-tauri/src/cli.rs`). Adding a new flag is a single `#[arg(long)]` field on `CliArgs` (or a variant on `PromptCmd`/`SkillCmd` / `Commands`); dispatch lives in `src-tauri/src/lib.rs` (`run()` / `run_headless_transcription()` / `single_instance` callback) and reusable logic in `src-tauri/src/signal_handle.rs`.

> Run `handy --help` or `handy <subcommand> --help` for the authoritative list. This file mirrors that output with extra context.

## Quick Reference

| Flag / Command | Description |
|---|---|
| `--toggle-transcription` | Toggle recording on/off (sent to running instance) |
| `--toggle-post-process` | Toggle recording with AI post-processing on/off |
| `--cancel` | Cancel the current operation |
| `--start-hidden` | Launch without showing the main window (tray stays) |
| `--no-tray` | Launch without system tray (closing window quits) |
| `--debug` | Enable Trace logging (runtime-only) |
| `-f, --transcribe-file <WAV>` | Headless transcription of a 16 kHz mono WAV and exit |
| `--model <ID>` | Model id for `--transcribe-file` (default: selected model) |
| `--device-index <N>` | Compute device index for `--transcribe-file` (`--list-devices`) |
| `--list-devices` | List transcribe-cpp compute devices and exit |
| `--list-models` | List available models and exit (honors `--json`) |
| `--repeat <N>` | Repeat transcription N times (reports best_ms) |
| `--json` | Emit `--transcribe-file` results as JSON |
| `prompt <subcmd>` | Prompt library: search/list/get/use/create |
| `skill <subcmd>` | Skill interop: export/add/find |

Remaining flags (`--help`, `--version`) are provided by clap.

## Remote Control vs Startup Flags

**Remote control flags** (`--toggle-transcription`, `--toggle-post-process`, `--cancel`) work via `tauri_plugin_single_instance`: a second `handy` process forwards its `CliArgs` to the already-running instance over a local IPC channel, then exits. Exit code 0 means delivered. If Handy was not running, they start normally and do nothing (no-op).

**Startup flags** (`--start-hidden`, `--no-tray`, `--debug`) are runtime-only overrides — they do **not** write to `settings_store.json`.

Server mode (`server_mode_enabled`, `server_port`, `server_auth_token`) is settings-only (Settings → Advanced → Server Mode, or Advanced → API). There is no `--server-port` CLI; the port is read from the store (portable-aware via `portable::store_path`) and the actual bound port is kept in `ServerState` (tries `preferred..+10`).

## Headless Transcription (`--transcribe-file`)

```bash
handy --transcribe-file ./recording.wav
handy --transcribe-file ./recording.wav --model parakeet-tdt-0.6b-v3 --json
handy --list-devices
handy --transcribe-file ./recording.wav --device-index 2
handy --list-models --json | jq
handy --transcribe-file ./recording.wav --repeat 5 --json
```

* No mic, no VAD, no download — the model must already be installed in the app data `models/` dir.
* Input must be **16 kHz mono 16-bit PCM WAV** (checked via `hound`); other formats should be pre-converted (`ffmpeg -i in.mp3 -ar 16000 -ac 1 -c:a pcm_s16le out.wav`).
* Runs the same `TranscriptionManager::transcribe_with_wav_path` the app uses; `--device-index` hard-selects the `transcribe-cpp` device by its `--list-devices` registry index (not persisted).

Exit codes: `0` ok, `1` runtime failure (load/transcription), `2` bad input / usage.

## Prompt & Skill CLI

```bash
handy prompt search "meeting notes" --limit 20 --json
handy prompt list --pinned --tag writing --json
handy prompt get 42 --raw --var k=v
handy prompt use "meeting" --stdout --copy
handy prompt create --title "My Prompt" --content "Hello {{name}}" --tag demo

handy skill export 42 --out ./skill.md --stdout
handy skill add ./skill.md --folder 1 --tag imported
handy skill find "summarize" --json
```

The prompt DB is `portable::app_data_dir()/prompt_library.db` (falls back to `~/Library/...`, `%APPDATA%`, or `~/.local/share`).

## Extending the CLI

Add a field to `CliArgs` in `src-tauri/src/cli.rs`:

```rust
#[derive(Parser, Debug, Clone, Default)]
#[command(name = "handy", about = "Handy - Speech to Text")]
pub struct CliArgs {
    /// My new flag
    #[arg(long)]
    pub my_flag: bool,
}
```

Handle it in `src-tauri/src/lib.rs` (`run()` → setup closure or `single_instance` handler) and route shared logic through `signal_handle::send_transcription_input` where applicable.

Clap's derive macros validate types, generate `--help`, and enforce `value_parser`, `ValueEnum`, and `Subcommand` constraints.

## API Server (`--` no CLI flag `—` settings only)

The local loopback API (127.0.0.1 + Bearer token) is documented in-app (Settings → Advanced → API) and via `GET /openapi.json`:

```bash
TOKEN=$(jq -r .server_auth_token ~/Library/Application\ Support/com.pais.handy/settings_store.json)
curl http://127.0.0.1:17373/v1/models -H "Authorization: Bearer $TOKEN"
curl -X POST http://127.0.0.1:17373/v1/audio/transcriptions \
  -H "Authorization: Bearer $TOKEN" \
  -F file=@audio.wav -F model="whisper-small"
```

See `src-tauri/src/openapi.json` and the in-app API card (copyable cURL / JS / Python examples) for details.

## Tray / Window Manager Integration (Linux / Wayland)

Use the CLI flags as the `Exec=` target for GNOME/KDE/Sway/Hyprland shortcuts:

```ini
# Sway / i3
bindsym $mod+o exec handy --toggle-transcription
bindsym $mod+p exec handy --toggle-post-process

# Hyprland
bind = $mainMod, O, exec, handy --toggle-transcription
```
