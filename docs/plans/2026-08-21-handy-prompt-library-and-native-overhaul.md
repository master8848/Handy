# Handy Prompt Library + Native Overhaul — Phased Plan

> Status: **expanded** — 4 sub-agents audited each phase 2026-08-21. Draft skeleton remains in §0-5 headers; deep dives appended per phase. See `## Trash / Cut Summary` for what to defer.
> Scope: turn Handy from single-purpose transcriber into prompt-library manager + voice capture, remove mandatory WebView, use native listening UI.
> Context: `AGENTS.md:40` architecture, `src-tauri/src/lib.rs:606` entry, `src-tauri/src/overlay.rs:361` overlay, `src-tauri/src/managers/prompt_history.rs:1` pasted prompts, `src/components/Sidebar.tsx:52` sections, `src-tauri/src/settings.rs:385` `AppSettings`.

## Goals

- Prompt library management as first-class addition (not replacement) to Handy transcription flow.
- Server / browser mode so settings/library need not spawn a WebView (Safari/WebKit) process when unused — saves 150-300 MB by using native browser + OS toolbar side panel (enabled in advance).
- Listening indicator migrated from WebView overlay (`src/overlay/`) to native SwiftUI (macOS) / WinUI (Windows) — `overlay.rs` becomes native bridge.

## Constraints

- Keep Tauri core, Rust managers, `tauri-plugin-store` settings flow (`settings.rs:1075`).
- i18n via `src/i18n/locales/en/translation.json` — no hardcoded strings.
- Single-instance (`lib.rs:841` `tauri_plugin_single_instance`) and `portable.rs` must keep working.
- Memory win measurable and opt-in (user enables in advance).

---

## Phase 0 — Foundation & Instrumentation

**Objective:** baseline metrics and scaffolding so later phases are verifiable.

- Measure current memory: WebView process RSS with/without main window (`lib.rs:917` `WebviewWindowBuilder`), overlay WebView (`overlay.rs:377`). Record on macOS (WKWebView) and Windows (WebView2). Use `ps -o rss`, Activity Monitor, Task Manager `Working Set`. Log `app.webview_windows().keys()` (`lib.rs:121`) count.
- Define feature flags in `settings.rs` (`AppSettings:385`): `prompt_library_enabled bool` (or just ship enabled), `server_mode_enabled bool`, `server_port u16` (17373), `server_bind String` (127.0.0.1), `server_auth_token Option<String>`, `overlay_native_enabled bool`. Defaults **off** (opt-in). Bump `CURRENT_SETTINGS_SCHEMA_VERSION:540` 1→2 in Phase 5.
- Add `docs/plans/` + `.agent/plans/` tracking; create `.agent/notes/implemented/` stub if applying `dsh-doc-standards` skill.

**Exit criteria:** numbers recorded, flags default off, no user-visible change.

---

## Phase 1 — Prompt Library Management (expanded by Agent A)

**Objective:** replace `Pasted prompts` (`src/components/settings/home/PromptHistory.tsx:1`) with full library. Competitive baseline (Tipinator, PromptMan, PromptHub, Langfuse, PromptLayer, AIPRM, TextExpander, Notion) → local-first library, folders, tags, pin, search, version history/diff/restore, markdown editor, variables `{{name}}`, import/export JSON, one-click insert via global shortcut.

### 1.1 Data model & SQL

Why new file: `prompt_history.rs:54` uses bare `CREATE TABLE IF NOT EXISTS` with no `user_version` migrations, while `history.rs:20` uses canonical `rusqlite_migration::Migrations`. Create `prompt_library.db` (or reuse `prompt_history.db` with migrations — Agent D recommends reusing file to avoid two portable files) via `history.rs:98` pattern. Keep legacy file until migration confirms.

`Cargo.toml:46` already has `rusqlite_migration 2.3` + `rusqlite bundled` + `regex 1` for `{{var}}` extraction — no new crates.

Schema (Migrations static, mirrors `history.rs:20`):

- `folders(id, name UNIQUE COLLATE NOCASE, color, sort_order, created_at)` + index on name.
- `prompts(id, title, content, folder_id FK SET NULL, pinned bool, usage_count, last_used, created_at, updated_at, version, variables_json)` + indexes on `updated_at DESC`, `folder_id`, `pinned`.
- `tags(id, name UNIQUE COLLATE NOCASE)` + `prompt_tags(prompt_id, tag_id PK)` + index on `tag_id`.
- `prompt_versions(id, prompt_id FK CASCADE, version UNIQUE per prompt, title, content, created_at)` + index on `(prompt_id, version DESC)`.
- Seed `Imported` folder for legacy migration path.
- Cap: prompts unbounded; versions pruned to **50 per prompt** on insert (unlike `MAX_PROMPT_HISTORY_ENTRIES 100` at `prompt_history.rs:13`).

Migration: after `to_latest`, if `prompt_history.db` exists and prompts count == 1 (seed only), `ATTACH DATABASE ? AS legacy` then `INSERT INTO prompts (title, content, folder_id, created_at, updated_at, variables_json) SELECT substr(prompt_text,1,80), prompt_text, (SELECT id FROM folders WHERE name='Imported'), timestamp, timestamp, '[]' FROM legacy.prompt_history ORDER BY timestamp DESC` in transaction. Keep legacy file.

Variables: derived on `create/update` via `Regex r"\{\{\s*([A-Za-z0-9_]+)\s*\}\}"` → `serde_json::to_string(&vars)` → `variables_json`. Expose `variables: Vec<String>` in API.

Connection helpers: `get_connection()` with `busy_timeout=5000`, `map_prompt/ folder/ version/ tag`. `update_prompt` in `BEGIN IMMEDIATE` transaction: if `old.title != new || old.content != new` then `INSERT INTO prompt_versions` + `UPDATE prompts SET version=version+1`.

### 1.2 Rust manager + Tauri commands

New `src-tauri/src/managers/prompt_library.rs` (`lib.rs:174` instantiate `Arc::new`, `lib.rs:187` `manage`, `managers/mod.rs:1` export):

- `Prompt { id, title, content, folder_id, folder_name, tags, variables, pinned, usage_count, last_used, created_at, updated_at, version }`, `Folder`, `PromptVersion`, `PromptLibraryUpdatePayload` (`Added/Updated/Deleted/Pinned`) mirroring `prompt_history.rs:127` emission.
- Methods: `create_prompt`, `get_prompt` (JOIN folders + `GROUP_CONCAT` tags), `list_prompts(filter: PromptFilter)` with `PromptSort = UpdatedDesc|CreatedDesc|UsageDesc|TitleAsc`, `update_prompt` (+ version), `delete/duplicate/toggle_pin/set_folder/increment_usage`, `list_folders/create/update/delete`, `list_tags`, `list_versions/restore_version`, `export_json/import_json`, `search` folded into `list_prompts`.

Commands file `src-tauri/src/commands/prompt_library.rs` (`commands/mod.rs:1`), each `#[tauri::command] #[specta::specta]` with `State<Arc<PromptLibraryManager>>`: `list_prompts`, `get_prompt`, `create_prompt`, `update_prompt`, `delete_prompt`, `duplicate_prompt`, `toggle_prompt_pin`, `increment_prompt_usage`, `insert_prompt(id, variables?)`, `list_folders/create/update/delete`, `list_tags`, `list_prompt_versions/restore`, `export/import`. Register in `lib.rs:620` `collect_commands!` + `lib.rs:757` `collect_events!` → `bindings.ts`.

`insert_prompt` uses existing paste path `clipboard::paste(rendered, app)` (`clipboard.rs:613`) respecting `paste_method`, `paste_delay_ms`, `reliable_paste` (`paste_tx`), then `increment_usage`. Validate `rendered.contains("{{")` → error unfilled.

### 1.3 Frontend

- Navigation: extend `AppSection` (`lib/types/navigation.ts:1`), add `SECTIONS_CONFIG["prompt-library"]` in `Sidebar.tsx:52` with `Library` icon (`lucide-react`), `sidebar.promptLibrary` i18n key (`translation.json:13`). Keep legacy history hidden after migration.
- State: `src/stores/promptLibraryStore.ts` Zustand `subscribeWithSelector` mirroring `settingsStore.ts:184`, subscribe to `events.promptLibraryUpdatePayload.listen` (like `PromptHistory.tsx:41`).
- Components `src/components/prompt-library/`:
  - `PromptLibraryView.tsx` (filter rail + searchable list, debounced 200ms, sort select, New/Import/Export)
  - `PromptCard.tsx` (title, clamp 2 lines content, folder/tag chips, pin `Star`, usage, actions: Insert/Copy/Edit/Duplicate/Delete with `Dialog` confirm)
  - `PromptEditor.tsx` (Title `Input`, Folder `Select` + inline new, Content `Textarea`/TipTap reuse from `Home.tsx:106`, `TagInput` autocomplete, variable chips live)
  - `VersionHistoryDrawer.tsx` (newest-first, diff via `diff` lib, Restore confirm)
  - `FolderManager.tsx` (color picker)
  - `VariableFillDialog.tsx` (one Input per `prompt.variables`, remembers `localStorage` per prompt)
  - `PromptPalette.tsx` (global fuzzy finder, see §1.4) mounted in `App.tsx` portal.

Insertion: `PromptCard` Insert → if `variables.length` open `VariableFillDialog` else `commands.insertPrompt(p.id, null)` with `sonner` toast. Substitute via pure `replace("{{k}}")` + `replace("{{ k }}")` — no expression eval.

### 1.4 Global palette shortcut

Add `prompt_palette` binding in `settings.rs:862` `get_default_settings()` (`option+shift+p` macOS, `ctrl+shift+p` else) via existing `settings.rs:81` `ShortcutBinding`. Auto-merged for existing users (`settings.rs:1097`). `shortcut/handler.rs:29` dispatches `binding_id == "prompt_palette"` → `app.emit("prompt-library:open-palette", ())` (+ `show_main_window`). Frontend `PromptPalette.tsx` listens, fetches `listPrompts`, filters via `fuse.js` or `search_prompts`. Phase 1 showing main window is acceptable; truly headless palette defers to Phase 2 server mode. Generic `change_binding` (`shortcut/mod.rs:112`) already handles remap.

### 1.5 Trash / Cut for Phase 1

| Feature | Verdict | Why |
|---|---|---|
| Cloud sync / team workspaces (PromptHub, Langfuse) | CUT | No auth/multi-user, adds GDPR. Keep export/import only. |
| LLM test playground / provider run | DEFER | Duplicates existing `LLMPrompt` post-processing (`settings.rs:421`). |
| Analytics dashboard / cost tracking | TRASH | `usage_count` + sort-by-usage suffices; charts vanity <5k prompts. |
| Abbreviation text-expander auto-replace | CUT | Conflicts with `clipboard.rs:613` paste matrix, Wayland issues. |
| Deeply nested folders | DEFER | Flat folders + tags minimal. |
| Notion kanban/calendar/table views | TRASH | List + filters enough. |
| Real-time collaboration | CUT | No sync. |
| AI prompt auto-optimize | TRASH | Needs LLM, non-deterministic scope creep. |
| Branching fork/merge | CUT | Linear `version` monotonic; branching is git overkill. |
| Rich HTML side-by-side diff | DEFER | Unified text diff + highlight suffices. |
| Tag colors/icons | TRASH | Colors for folders only. |
| Prompt evaluation / A/B harness | TRASH | Belongs in Langfuse. |

**Exit criteria:** CRUD + tag/folder/version/search/insert via paste works; legacy 100-entry log migrated to `Imported`; spec `updatedDesc` default.

**Ordered steps (A):** `settings.rs` flag → `prompt_library.rs` manager (clone `history.rs:20` pattern) → `managers/mod.rs` + `lib.rs:174/187` → `commands/prompt_library.rs` → `settings.rs:862` palette binding + `shortcut/handler.rs` → `Sidebar.tsx` + `translation.json` + `promptLibraryStore.ts` → `src/components/prompt-library/*` → `PromptPalette.tsx` in `App.tsx` → import/export + tests (in-memory DB like `prompt_history.rs:180`) → manual QA (5 prompts, variable insert, version restore, palette hotkey).

---

## Phase 2 — Server Mode (expanded by Agent B)

**Objective:** user can run Handy without any WebView; management via native browser + toolbar side panel. WebView spawns lazily on demand. Saves 150-300 MB (main window) + 30-80 MB (overlay) when combined with Phase 3.

### 2.1 Memory model (honest)

| Window | Process | RSS (empty) | Removable by Phase 2 |
|---|---|---|---|
| `main` (`lib.rs:917` `WebviewWindowBuilder` `frontendDist: ../dist` `tauri.conf.json:10`) | WKWebView (macOS `WebKit.framework`), WebView2 `msedgewebview2.exe`, WebKitGTK (Linux in-process) | ~120-200 MB | **Yes** — skip builder when `server_mode_enabled` |
| `recording_overlay` (`overlay.rs:363`/`424` `src/overlay/index.html` `vite.config.ts:22` second input) | second renderer | ~40-80 MB | **Phase 3** native, not Phase 2 |
| Tray only (`TrayIconBuilder:228` `tray.rs:228`) | no renderer | ~5 MB | — |

Phase 2 alone saves `main` only; full 150-300 MB saving needs Phase 3. Overlay WebView was source of unbounded `wry` `eval_script` growth `overlay.rs:640` even with `OVERLAY_ENABLED=false` guard (`#1279`).

### 2.2 Architecture choice

`axum 0.7` + `tower-http` recommended over `tiny_http` or `tauri-plugin-http`. `tiny_http` saves ~1 MB but reimplements `ServeDir` Range/CORS; `tauri-plugin-http` would expose all 70+ `#[tauri::command]` (`lib.rs:621`) including `open_app_data_dir` — too broad. `axum` reuses `tokio` already at `Cargo.toml:67` (currently no features — add `rt-multi-thread`). Dist resolution: prod `app.path().resource_dir().join("dist")`, dev `../dist/index.html` relative to `src-tauri/` or 503 instructing `bun run build`. SPA fallback via `tower_http::services::ServeDir` + `ServeDir::fallback(ServeFile::new(dist/index.html))` for React Router.

### 2.3 Startup branching (`lib.rs:606 run()`)

Inside `.setup(|app| { let settings = get_settings(app.handle()); if settings.server_mode_enabled { // skip WebviewWindowBuilder, keep initialize_core_logic, spawn axum on tokio } else { // existing WebviewWindowBuilder:917 } })`. Keep `initialize_core_logic` (`lib.rs:154`) for tray + `TranscriptionCoordinator` in both branches; skip `create_recording_overlay` if Phase 3 native already. On macOS set `ActivationPolicy::Accessory` (`lib.rs:219/1013`) when server mode start_hidden.

Add `ensure_main_window(app)` helper for lazy creation reused by tray and single-instance.

`dist_dir`: serve `dist/assets/main-*.js (958K)` before fallback; exclude `dist/src/overlay/index.html (705B)` not needed in server mode. PWA `dist/manifest.json` served for installability (`theme_color #ec4899`).

### 2.4 Tray + single-instance

`src-tauri/src/tray.rs:184` `update_tray_menu`: add `open_in_browser` (`http://127.0.0.1:17373`) via `app.opener().open_url` (`tauri_plugin_opener` `Cargo.toml:40`), `open_settings_window` (lazily builds `WebviewWindowBuilder` on demand, idempotent). Menu order: version → open_in_browser → open_settings_window → separator → model_submenu.

`lib.rs:842` `single_instance` callback: if `server_mode_enabled && get_webview_window("main").is_none()` then plain launch → `opener.open_url(local_url)` instead of `show_main_window`; `--toggle-*` / `--cancel` still → `signal_handle::send_transcription_input:18` / `utils::cancel_current_operation`. Keep `headless_mode` guard (`lib.rs:776`).

Auth guard: public `GET /`, `/assets/*`, `/manifest.json`, `/api/health`; else require `Authorization: Bearer <server_auth_token>` (generated `base64url(32 random)` via `rand`, stored `Option<String>` in settings, rotated via settings reset). CORS allow `http://127.0.0.1:{port}` + `http://localhost:{port}` only.

### 2.5 Toolbar side panel (docs, not code)

No native sidebar to implement — pinned URL:

- macOS: `http://127.0.0.1:17373` → Chrome/Edge `⋮ → Save and Share → Install page as app` (PWA) or Safari `File → Add to Dock` (macOS 14+) or `edge://sidebar` `+`.
- Windows: same URL; Edge `edge://sidebar` or Chrome `Customize Chrome → Side panel → Add site`.
- Provide `public/manifest.json` and `<link rel="manifest">` in `dist/index.html`.

### 2.6 Port handling

Default 17373 `settings.rs:385`. On `start()` try bind; on `AddrInUse` iterate `17373..17383` before failing. Actual bound port in `ServerHandle.port` (ephemeral, logged), preferred port stays in settings. Tooltip `tray.rs:172` appends `127.0.0.1:port`. Optional CLI `--server-port` runtime-only override (pattern `lib.rs:942 --debug`).

### 2.7 Trash / Cut for Phase 2

| Cut | Why |
|---|---|
| Custom bind `0.0.0.0` / LAN | Security regression — exposes history/library to LAN. Keep `127.0.0.1` hard-coded. |
| Remote TLS / beyond Bearer | Over-engineering for loopback MVP. |
| SSE/WebSocket for live text | No consumer until Phase 3 native overlay; defer. |
| Full `__TAURI_IPC__` bridge | Exposes 70+ commands (`initialize_enigo`, `open_app_data_dir`, `set_log_level`) — whitelist only. |
| Browser extension | Maintenance + store review for a pinned URL — docs only. |
| Vite dev server proxy | Require `bun run build` for server-mode testing. |
| `tiny_http` hand-roll | Saves 1 MB, reimplements `ServeDir` correctly — not worth it. |

**Exit criteria:** toggle on + restart → `app.webview_windows().keys()` contains only `recording_overlay` (or none with Phase 3), no `main`; tray + hotkey `transcribe` (`settings.rs:872`) still fires; browser `http://127.0.0.1:port` manages prompts/settings; lazy `Open Settings Window` creates `main` on demand; second-instance forwarding opens browser not dead `show_main_window`.

**Ordered steps (B):** measure RSS → `settings.rs` fields + `CURRENT 1→2` migration + `get_default_settings` token generation → `Cargo.toml` `axum/tower-http/tower` + `tokio rt-multi-thread` → `server.rs` (`start/stop/local_url`, `ServeDir` + health + auth) + `public/manifest.json` → `lib.rs` startup branching + `ensure_main_window` → `tray.rs` items + `lib.rs:271` `on_menu_event` + `tray_i18n` keys → `single_instance` branch → frontend `ServerSettings.tsx` section (toggle + port + token copy + warning "Requires restart") → QA matrix (macOS 13+/Win10/11/Linux Wayland, `HANDY_NO_GTK_LAYER_SHELL=1`).

---

## Phase 3 — Native Listening UI (expanded by Agent C)

**Objective:** recording/listening indicator uses native UI, not WebView. Replace `src/overlay/RecordingOverlay.tsx:1` + `RecordingOverlay.css` WebView with native.

### 3.1 Current cost

`overlay.rs:363`/`424` `PanelBuilder`/`WebviewWindowBuilder` + `src/overlay/*` → second renderer (~40-80 MB), audio callback `managers/audio.rs:292` → `overlay.rs:672` `emit_to("recording_overlay","mic-level",levels)` → `wry` IPC → `webkit eval_script` → React state → CSS transition 80ms (`RecordingOverlay.css:320`). Throttle `EMIT_THROTTLE_MS 33` caps at ~30fps. `calculate_overlay_position:238` fights `tao` DPI bugs (`work_area.position.y` `tauri 2.11`, `windows_overlay_bounds:287` bypass), `force_overlay_topmost:142` `SetWindowPos(HWND_TOPMOST)` required, Linux `init_gtk_layer_shell:115` fails silently on compositors without layer-shell. `hide_recording_overlay:608` spawns thread sleep 300ms for JS fade — hazard on exit.

### 3.2 Trade-off

Native: no renderer (macOS `NSPanel`+`NSHostingView` ~2-5 MB, Windows `HWND`+`softbuffer` ~1 MB), mic levels via direct FFI (no IPC), waveform at 60/120 Hz vs 30, canonical topmost primitives (`NSPanel Level::Status:433` `StyleMask::nonactivating_panel:442` `CollectionBehavior:445`), fewer workarounds. Costs: Swift toolchain on macOS (Xcode), custom 2D renderer on Windows, theme-token duplication (`theme.css:1` + `RecordingOverlay.css:19`), preview slower than `bun run dev` HMR. Decisive factor is `#1279` `wry` leak and zero-WebView goal — last WebView; removing it unlocks full memory win.

### 3.3 macOS native design

Target:

```
Rust (overlay::native_macos)  →  Swift (src-tauri/native-overlay/macos)
overlay.rs façade → NativeOverlay trait → RecordingPanel.swift
 ├─ show/hide/emit_levels          ├─ NSHostingView<RecordingView>
 └─ objc2 / tauri-nspanel bridge   └─ NSPanel (level .status, nonActivating)
       ↑ build.rs swiftc ─────────→ liboverlay_macos.a
```

Keep `tauri-nspanel` (`Cargo.toml:172` git `v2.1`) for `PanelBuilder` plumbing if it can host `NSView` via `with_window(|w| ...)` + `contentView` injection; else raw `objc2`/`objc2-app-kit` (`Cargo.toml:178`) create `NSPanel` `*mut NSPanel` in `OnceLock`. Prefer `tauri-nspanel` shim; fallback raw `objc2` ~60 lines.

Swift module `src-tauri/native-overlay/macos/`:

- `RecordingPanel.swift` `struct RecordingView: View` with `@State` for `OverlayState`, 9 bars `WAVE_BARS:26`, `streamText (committed/tentative/phase/workKind)`, `elapsed`, `position Top/Bottom`, `overflowing`. `TimelineView`/`CADisplayLink` lerp (`prev*0.7+target*0.3` + `height max(3,min(18,3+pow(v,0.7)*15))` `RecordingOverlay.tsx:249`).
- `overlay_bridge.h` exposing `overlay_show(const char* state)`, `overlay_hide()`, `overlay_set_levels(float*, size_t)`, `overlay_set_stream_text(...)`.
- `overlay_panel.swift` `@_cdecl` dispatching to `MainActor`.

Styling map tokens to SwiftUI (`RecordingOverlay.css:19` → Swift, see §3.5 table). Use opaque `Color(nsColor:.windowBackgroundColor)` + `RoundedRectangle(cornerRadius: 24/16/18)` matching `scard:119`; avoid translucency unless measured.

Rust shim `src-tauri/src/overlay/native_macos.rs` (`#[cfg(target_os="macos")]`) with same 7 symbols `create/show_*/hide/update_position/emit_levels/update_overlay_enabled_cache` (`overlay.rs:461/546/550/555/565/608/632/636`). Reuse `calculate_overlay_position:238`, `OVERLAY_WIDTH/HEIGHT:46`, `STREAM:50`, `overlay_dimensions:54`, `OVERLAY_TOP_OFFSET:66`. `NSPanel.setFrameOrigin(NSPoint(x,y))` after logical points. Lift throttle to 16ms (60fps) allowed (one FFI vs WebView IPC). Hide via `NSAnimationContext` fade + `panel.orderOut(nil)`, not thread sleep.

Event bridge: `show_*` calls `overlay_show("recording"|"streaming"|...)` + position arg from `get_settings(app).overlay_position` (eliminates JS `commands.getAppSettings:76` round-trip). `emit_levels` → `overlay_set_levels`. Streaming: keep `StreamTextEvent`/`StreamPhaseEvent` (`managers/transcription.rs:62`) events for WebView fallback, additionally call FFI when native enabled.

Build: `build.rs` new `build_overlay_panel()` mirroring `build_apple_intelligence_bridge:387`/`build_os_speech_bridge:568` (`SDKROOT`/`SWIFTC` via `xcrun`, `-parse-as-library -target arm64-apple-macosx11.0 -sdk $SDKROOT -O`, `libtool -static -o liboverlay_macos.a`, `cargo:rustc-link-lib=static=overlay_macos`, `rerun-if-changed`). No extra entitlements; static lib not signed separately. Require full Xcode (not CLT) — fallback stub hide overlay if `is_command_line_tools_only:436`.

### 3.4 Windows native design

Rendering stack choice:

| Candidate | Verdict |
|---|---|
| `softbuffer` + `tiny-skia` | **Recommended** — pure Rust, CPU raster negligible for 256×46–400×120, smallest delta. |
| `windows-rs` GDI manual `UpdateLayeredWindow` | Viable alternative (zero extra crates beyond `windows 0.61.3:129`). |
| `wgpu` / GPU | Reject — overkill, risks Vulkan conflict (`transcribe-cpp:163`). |
| WinUI 3 / XAML Islands | Reject — packaging/`AppX`, breaks portable `portable.rs:14`. |
| `winit` + `softbuffer` | Reject — duplicates `tao` windowing (`Cargo.toml:206` fork). |

Reuse `windows_overlay_bounds:287` + `place_windows_overlay:315` verbatim. `HWND CreateWindowExW(WS_EX_LAYERED|WS_EX_TOPMOST|WS_EX_TOOLWINDOW|WS_EX_TRANSPARENT|WS_EX_NOACTIVATE, WS_POPUP, x,y,w,h)` + `SetLayeredWindowAttributes` + `SetWindowPos(HWND_TOPMOST, SWP_SHOWWINDOW)` after `ShowWindow(SW_SHOWNOACTIVATE)`. Class `RegisterClassExW` once. Keep two-phase placement (`place_windows_overlay` before after `ShowWindow` to survive `WM_DPICHANGED:528`). Click-through via `WS_EX_TRANSPARENT|WS_EX_NOACTIVATE` mirrors `focusable(false):394`.

Rendering: pill flat `RecordingOverlay.css:129` no shadow; live panel `scard.open 392 radius 16:156` / `working 216 radius 18:159` mapped to `RoundRect`; text `font 15px italic:204` `Segoe UI` via `DrawTextW`/`cosmic-text`; waveform 9 bars `4px` `gap 3px:306` `height 3+pow(v,0.7)*15:249` smoothed `prev*0.7+target*0.3` moved native; timer `mm:ss` `stimer:293` `SetTimer`; cancel `sx 22px:322` hit-test `WM_LBUTTONDOWN` → `utils::cancel_current_operation:76`.

`Cargo.toml` add `softbuffer 0.4` + `tiny-skia 0.11` under `[target.'cfg(windows)'.dependencies]` (pin compatible with `windows 0.61.3`). No `build.rs` change. Signing `tauri.conf.json:73` covers static HWND code.

### 3.5 Linux fallback

Linux stays WebView + `gtk_layer_shell:114` (`Layer::Overlay:129`, `is_supported:121`). Reason: compositor diversity (Wayland `LayerShell` vs X11 `_NET_WM_STATE_ABOVE`) and native `winit/softbuffer` on Wayland needs `Smithay`/`gtk`. Also `settings.rs:590` `default_overlay_style()` already `OverlayStyle::None` on Linux (overlay hidden) → `OVERLAY_ENABLED=false:627` + `emit_levels:645` early return mitigates `wry` concern. Flag `overlay_native_enabled` defaults `false` on Linux. `current_overlay_logical_size:275` + `update_gtk_layer_shell_anchors:77` remain Linux-only. `HANDY_NO_GTK_LAYER_SHELL=1:116` still works.

### 3.6 Façade preservation

Keep 7 public symbols signatures so `lib.rs:354`, `utils.rs:76`, `managers/audio.rs:292`, `shortcut/mod.rs:681`, `transcription_coordinator.rs` unchanged: `create_recording_overlay`, `show_recording_overlay`, `show_streaming_overlay`, `show_transcribing_overlay`, `show_processing_overlay`, `update_overlay_position`, `hide_recording_overlay`, `update_overlay_enabled_cache`, `emit_levels`. Introduce `mod native_macos`/`native_windows`/`webview` in `src-tauri/src/overlay/mod.rs` (rename current file), move shared constants (`OVERLAY_WIDTH:46`, `STREAM:50`, `overlay_dimensions:54`, `calculate_overlay_position:238`, `windows_overlay_bounds:287`, `place_windows_overlay:315`, `get_monitor_with_cursor:170`, `OVERLAY_TOP_OFFSET:66`) to `mod.rs`.

Dispatch:

```rust
fn is_native_enabled(app: &AppHandle) -> bool { get_settings(app).overlay_native_enabled }
pub fn create_recording_overlay(app: &AppHandle) {
  if is_native_enabled(app) { #[cfg(target_os="macos")] return native_macos::create(app); #[cfg(windows)] return native_windows::create(app); }
  webview::create(app)
}
```

Optional compile-time feature `native-overlay` default-on for macOS/Windows; runtime flag for safe rollback.

Events: keep `emit_to("recording_overlay","mic-level")` + `emit("show-overlay")` etc. Native additionally calls FFI; WebView listeners receive nothing when native active (window absent).

### 3.7 Visual parity spec (condensed)

`RecordingOverlay.css:19` tokens → native. Window `OVERLAY_WIDTH 256 / STREAM 400×120 :50` must fit card `ov-work-w 216 / ov-open-w 392 :28` + 40 slack. `top 46 / bottom 15` macOS, `4 / 40` WinLinux (`overlay.rs:66`). Tokens: `--s-surface 98% background:7`, `--s-accent #faa2ca/#f28cbb:9` + `AccentColor` overrides `theme.css:60`, `--ov-base-h 40:34` row, `--ov-cap-max-h 64:35` clip, `scard radius 24:119` `open 16:156` `working 18:159`, `pop 460ms cubic-bezier(0.22,1,0.36,1):137`, `stext grid 0fr→1fr 440ms:178`, `sdot 7px:277` pulse `1.9s:285`, `swave 4px bars gap 3px:306` 80ms transition, `sx 22px:322` X `1.6 round:262`, `sspinner 13px 0.7s:357`, `stext-cap 15px italic 1.35:204` mask fade `18px:209`, wavy underline `var(--color-error):230`, caret `2px accent blink 1.05s:243` hidden when `working:334`. Non-negotiable: centered pill, correct per-state width, waveform, timer/cancel, streaming committed/tentative, top/bottom placement. Animations polish deferrable.

### 3.8 Trash / Cut for Phase 3

| # | Element | Why low-value |
|---|---|---|
| Spell wavy in native overlay (`SPELL_CHECK_DEBOUNCE 300:30` Harper) | Transient 64px underline for ~2s, not actionable in non-activating panel. Keep spell in editor. | Cut v1 |
| Timer in streaming open only | Time not actionable; `Minimal` vs `Live` already gates. | Keep but not animated |
| WinUI 3 / acrylic/blur | Flat opaque desired (`RecordingOverlay.css:129`); acrylic reintroduces translucency + `DWM`. | Reject flat |
| `wgpu`/GPU | Overkill 400×120; conflicts Vulkan. | Reject |
| `swift-bridge` codegen | Bare `swiftc` precedent suffices. | Reject |
| `grid 0fr→1fr` native animation | CSS grid trick; native measure per chunk costly. | Simplify `maxHeight 0→64` |
| `overflowing` mask + scroll pinning (`pinnedRef:61`, `overflowing:52`) | Cap rarely overflows (>4 lines); dictating not reading. | Drop conditional mask |

**Exit criteria:** macOS/Windows flag on → no `recording_overlay` WebView; native panel at correct `calculate_overlay_position`/`windows_overlay_bounds` monitor/position, animates show/hide, displays mic levels (+ streaming text if phase kept).

**Ordered steps (C):** `settings.rs:385` `overlay_native_enabled` + `default` `cfg(not(linux))` → `src-tauri/src/overlay/mod.rs` façade + `webview.rs` split + `native_macos.rs`/`native_windows.rs` stubs → macOS Swift module `native-overlay/macos/*` + `build.rs` + `native_macos.rs` FFI → Windows `CreateWindowExW` + `softbuffer` + hit-test + `WM_DPICHANGED` → Linux gate + settings UI toggle (disabled tooltip on Linux) → visual parity pass vs `RecordingOverlay.css` screenshots → keep `src/overlay/*` fallback for one release → `AGENTS.md:40` + `BUILD.md` Swift/`softbuffer` notes.

---

## Phase 4 — Spellcheck & Search Refinements (expanded by Agent D)

**Objective:** refine search and Image 2 spell popup for new library + native overlay.

### 4.1 Search: LIKE → FTS5

Current: `prompt_history.rs:54` single table, `PromptHistory.tsx:56` JS `includes()`, `commands/prompt_history.rs:42` → list then client filter. `history.rs:20` already `rusqlite_migration`; `prompt_history.rs` does not — fix.

`rusqlite bundled` at `Cargo.toml:78` enables `SQLITE_ENABLE_FTS5` — verify `SELECT sqlite_compileoption_used('ENABLE_FTS5')`. No extra feature.

Schema (reuse `prompt_history.db` migrated via `rusqlite_migration v2..3`, or keep separate `prompt_library.db` — reuse preferred for portable):

- `prompt(id, title, content, folder_id, pinned, usage_count, last_used, created_at, updated_at)` + `folder`, `tag`, `prompt_tag`, `prompt_version`.
- `CREATE VIRTUAL TABLE prompt_fts USING fts5(title, content, tags, content='prompt', content_rowid='id', tokenize='unicode61 "remove_diacritics 1"')` + triggers `prompt_ai/ad/au` + `prompt_tag` trigger + backfill `INSERT INTO prompt_fts(prompt_fts) VALUES('rebuild')`.

Ranking:

```sql
SELECT p.*, rank FROM prompt_fts JOIN prompt p ON p.id=prompt_fts.rowid
WHERE prompt_fts MATCH :q AND (:folder IS NULL OR p.folder_id=:folder)
ORDER BY p.pinned DESC, rank, p.updated_at DESC LIMIT 50;
-- rank = bm25(prompt_fts, 10.0, 5.0, 1.0)
```

`highlight(prompt_fts,1,'<mark>','</mark>')` for preview; sanitize via `ReactNode[]` like `RecordingOverlay.tsx:206` committedNodes (never `innerHTML`).

Perf: FTS5 `<15ms` cold / `<8ms` warm for 5k ×800ch, JS fallback ~4-6ms scan + 30ms IPC 4MB payload, index ~5MB. Wrap FTS in `spawn_blocking` like `commands/spellcheck.rs:27`. Fallback to `LIKE '%q%'` on `MATCH` syntax error (`"` unclosed).

Deployment: Step A land `Migrations` init keep legacy `list_prompt_history` + JS filter compat; Step B migrate 100 entries → `Imported`; Step C `PromptLibraryView` uses new command gated by flag; rollback `REINDEX`/`rebuild`.

### 4.2 Spellcheck

Current: `spellcheck.rs:9` `harper-core 2.7` (`concurrent`+`thesaurus`) `SpellChecker:41` `FstDictionary::curated(), Dialect::American:80`, `SpellingIssue:24` UTF-16 offsets, `commands/spellcheck.rs:22` async via `spawn_blocking`, `Home.tsx:238` + `RecordingOverlay.tsx:180` both `SPELL_CHECK_DEBOUNCE 300` (`Home.tsx:46`, `RecordingOverlay:30`), `spellCheckExtension.ts:56` `SpellCheckExtension` → `DecorationSet` → `Home.tsx:296` issueAtPoint popover, overlay `RecordingOverlay.tsx:206` committedNodes wavy underline.

| Axis | Harper | macOS `NSSpellChecker` | Windows Spellcheck API |
|---|---|---|---|
| Coverage | offline grammar+spelling, curated, English-only (`spellcheck.rs:224` CJK test) | system dicts many langs, iCloud | Win10+ `ISpellCheckerFactory` COM |
| IPC | one `check_spelling` | ObjC `objc2:0.6` `checkSpellingOfString:startingAt:` | COM apartment |
| Editor (TipTap, stays WebView) | **Wins** — unified, `spellCheckExtension.ts` mature | Needs `NSTextView` to get native menu — would replace TipTap, loses toolbar `Home.tsx:537` | Chromium built-in + custom underline duplicate |
| Native overlay (SwiftUI/WinUI) | IPC fine v1; reuse `SpellingIssue[]` event | Swift call sync, no IPC, `NSMenu` on `NSPanel` | COM heavy, defer |

Decision: **Editor stays Harper.** Native overlay stays Harper over IPC v1; if latency shows migrate macOS overlay to local `NSSpellChecker` sync (keep Harper for editor), Windows defers indefinitely. Extract `SPELL_CHECK_DEBOUNCE 300` to `src/lib/constants.ts` shared; native reuses 300ms + `checkSeqRef:66` invalidation. Add `SEARCH_DEBOUNCE 150` for FTS input.

### 4.3 Trash for Phase 4

- Native spellcheck in editor (TipTap stays WebView) — defer indefinitely.
- Inline suggestion popover inside native overlay streaming text (non-activating panel `PanelBuilder no_activate:440`).
- Ranking beyond `bm25`+`pinned` (no `strsim`/`natural`).
- Search-as-you-type without 150ms debounce.
- `SEARCH_DEBOUNCE` tunable setting.

**Exit criteria:** FTS `search_prompts(query,filters)` + `highlight` feels instant (<15ms 5k), LIKE fallback on syntax error; `SPELL_CHECK_DEBOUNCE` shared; native overlay spec defined (no code yet).

---

## Phase 5 — Integration, Migrations & Rollout (expanded by Agent D)

**Objective:** tie phases together, handle edge cases, document.

### 5.1 Settings migration

`CURRENT_SETTINGS_SCHEMA_VERSION 1:540` → **2**. Add fields `prompt_library_enabled`, `server_mode_enabled`, `server_port`, `server_bind`, `server_auth_token`, `overlay_native_enabled` (`#[serde(default)]` 385). Defaults off except `overlay_native_enabled` `true` on macOS/Windows (use `#[cfg(target_os)]`). Migration in `apply_settings_migrations:1184`:

```rust
if stored < 2 {
  // missing flags stay default via serde(default); overlay_native set per-platform
  settings.settings_schema_version = 2; updated = true;
}
```

Keep `stored <1` GPU reset before `<2`. Extend frozen fixture `settings.rs:1322` to assert old store parses/migrates. Keep `overlay_position="none"` → `OverlayStyle::None:1244` map; `overlay_style:None` disables WebView and native (cache `overlay.rs:627` `OVERLAY_ENABLED`).

### 5.2 DB migration

Migrate `prompt_history.rs:54` to `Migrations` (copy `history.rs:98`). New `prompt Library` `MIGRATIONS[1]` = `prompt/folder/tag/prompt_tag/prompt_version` + `prompt_fts` + triggers + `rebuild`; future `M3` for `variables JSON`. Portable path `portable::app_data_dir:60` + `Data/prompt_history.db` handled zero extra (`PromptHistoryManager::new:42` already portable-aware). Don't create separate `prompt_library.db` if reusing file.

### 5.3 Portable + CLI + single-instance

- Portable (`portable.rs:14` `Data/` + marker): server `port` in `settings_store.json` at `store_path:86` (already portable), no `Data/port.txt` needed; DB file location via `portable::app_data_dir`; `lib.rs:926` `data_directory` still holds for WebView fallback.
- CLI (`cli.rs:6` `CliArgs`, `lib.rs:942` `--debug` runtime-only override pattern): don't persist `CliArgs:server_port` to store (violates `AGENTS.md:176` runtime-only). Single-instance (`lib.rs:841`): if `server_mode_enabled && get_webview_window("main").is_none()` then plain launch → `opener.open_url(local_url)` instead of `show_main_window`; `--toggle-*`/`--cancel` still forward via `signal_handle::send_transcription_input:18`. Server HTTP API (`127.0.0.1:port` Bearer) not reused for CLI pipe — separate channel no conflict. `--start-hidden` + server mode don't double-hide (skip builder already satisfies).

### 5.4 Docs

`AGENTS.md:40` arch list `server.rs`, `managers/prompt_library.rs`, `overlay/native_*`, `AGENTS.md:159` CLI table add note `server_mode_enabled` settings-only + forwarding behavior, `BUILD.md:14` macOS Swift (Xcode 15+ for `tauri-nspanel:2.1`), `BUILD.md:35` Windows note `softbuffer` not `wgpu`, `AGENTS.md:184` Platform Notes `HANDY_NO_GTK_LAYER_SHELL=1`. `translation.json:782` add `promptLibrary/search` keys (no hardcode per `AGENTS.md:122`).

### 5.5 QA matrix

| Area | Matrix | Pass Criteria |
|---|---|---|
| macOS 13+ SwiftUI overlay | 13/14/15 Intel+ARM flag on/off | Native on: 0 `recording_overlay` WebViews, `NSPanel` topmost across Spaces `CollectionBehavior:444` |
| Win10/11 layered window | Win10 22H2, Win11 24H2, 100/150% mixed-DPI negative origins `overlay.rs:748` | Native on: 0 WebView2 overlay, `SetWindowPos` topmost, `windows_overlay_bounds:287` bounds logged `3648,2031,384,69` |
| Linux Wayland fallback | GNOME Wayland `gtk_layer_shell::is_supported:121`, `HANDY_NO_GTK_LAYER_SHELL=1:116`, Sway | Flag off → WebView overlay, `update_gtk_layer_shell_anchors:77` still called, no GDK-thread crash `show_overlay_state:481` hop |
| Memory before/after | `ps -o rss`, Activity Monitor, Task Manager WS | Server mode −150-300MB, native overlay −30-80MB steady-state |
| Search 5k seeded via import | `search_prompts("hello")` 10× | p50 <15ms FTS, p95 <30ms; LIKE fallback <50ms |
| Spellcheck overlay | Harper 500ch | p50 <20ms, debounce 300ms, stale guard `checkSeqRef:66` prevents ghost underlines |
| Single-instance + server | `--toggle-transcription`, `--cancel`, plain open second instance | All forward correctly, no spurious WebView creation |
| Portable | `Data/` + marker `portable.rs:14`, server mode on, search | All files under `Data/`; no `%APPDATA%` write |
| Migrations | Upgrade 0.9.4 fixture `settings.rs:1322` → new schema | No data loss, `settings_schema_version` 1→2 |

### 5.6 Trash / Cut for Phase 5 + Overall

| Cut | Why |
|---|---|
| Cloud sync for library | Defer; local-first niche, adds auth/conflicts. |
| `Data/port.txt` for side-panel discovery | Token+URL in Debug panel sufficient. |
| `tauri-plugin-store` replacement | Already handles `settings_store.json` + portable. |
| Keeping both `prompt_history` and `prompt_library` UIs | Alias `list_prompt_history` to empty after migration; don't ship two UIs. |
| Linux native overlay (`winit/softbuffer` Wayland) | `gtk_layer_shell` + `gtk:0.18` forever; Wayland layer-shell positioning niche. |
| `--server-port` CLI persistent | Violates runtime-only design `AGENTS.md:176`. |
| Opt-out native telemetry | No infra `memory.rs` none; manual QA suffices. |
| Full-text re-ranking beyond `bm25`+`pinned` | `strsim:74`/`natural:75` overkill. |
| Search-as-you-type 0ms / tunable `SEARCH_DEBOUNCE` | 150ms sweet spot, one constant. |

**Ordered steps (D 4+5):** FTS5 compile check (`sqlite_compileoption_used('ENABLE_FTS5')`) → migrate `prompt_history.rs` to `Migrations` → `prompt_library` tables + FTS5 + triggers + `rebuild` → `search_prompts` (`spawn_blocking`, BM25, filters, `highlight`) + `LIKE` fallback → frontend `SEARCH_DEBOUNCE 150` hook + safe marks → extract `SPELL_CHECK_DEBOUNCE 300` shared const → native spell rendering spec (Swift `AttributedString` wavy, WinUI `TextDecoration` via events) → settings bump 1→2 + `apply_settings_migrations` + frozen test extend → portable validation (`app_data_dir:60`, `data_directory:926`) → single-instance branch `lib.rs:841` → docs (`AGENTS.md` arch, CLI, `BUILD.md`, `translation.json`) → QA harness (`docs/plans/memory-measure.sh`, 5k seed import, timing log `liveLogs:520`).

---

## Trash / Cut Summary (all phases)

**Definite cuts (do NOT build):**
- Cloud/team sync, permissions, sharing, real-time collaboration — export/import JSON only.
- LLM playground / provider-run inside library — covered by post-processing prompts.
- Analytics dashboard / charts / cost tracking — `usage_count`+sort suffices.
- Text-expander abbreviation auto-replace — conflicts paste matrix, Wayland.
- Deeply nested folders, Notion kanban/table views, branching/merge, per-prompt permissions.
- Full `__TAURI_IPC__` HTTP bridge (expose 70+ commands) — whitelist only `prompt_library`/settings.
- LAN bind `0.0.0.0`, TLS for loopback, bearer beyond `127.0.0.1`, `tiny_http` hand-roll.
- Browser extension for toolbar — pinned URL + PWA manifest only; docs explain pinning.
- `wgpu`/GPU for Windows pill, WinUI 3/acrylic, `swift-bridge` codegen, `grid 0fr→1fr` native animation, conditional `overflowing` mask, wavy spell inside native overlay (defer).
- Native spellcheck in editor (TipTap stays Harper), Windows Spellcheck API (COM) for overlay, FTS re-ranking beyond `bm25`, 0ms search, tunable debounces, `Data/port.txt`, Linux native overlay, `--server-port` persistent CLI, telemetry opt-out.

**Deferred (not v1, revisit if demanded):**
- Variable expression eval (keep pure `{{name}}` replace).
- `tiny-skia` vs GDI choice refinement on Windows (prototype both).
- Swift `NSSpellChecker` sync in macOS overlay (keep Harper IPC v1, migrate if latency shows).
- CSV import (JSON only v1).
- `softbuffer` layered-window alpha translucency validation (flat opaque v1).

---

## Implementation Order & Dependencies

```
0 Foundation (measure, flags) ──┬──→ 1 Prompt Library (DB/manager/commands/UI) ──┐
                                │                                                  │
                                ├──→ 2 Server Mode (axum, tray, lazy window) ────┤──→ 5 Integration (migrations, portable, single-instance, docs, QA)
                                │                                                  │
                                └──→ 3 Native Overlay (NSPanel SwiftUI, HWND) ───┘
                                                   │
                                                   └──→ 4 Search+Spell (FTS5, debounce shared) ──┘
```

- No cross-block hard dependency except `5` needs all flags defined in `0`.
- `1` and `2` and `3` can develop in parallel branches; `4` needs `1` schema landed.

**Per-phase budgets:** Phase 1 ~5-6d, Phase 2 ~4-5d, Phase 3 ~6-8d (Swift + Win), Phase 4+5 ~4-5d → total ~19-24 engineer-days across 4 tracks.

## Open Questions — Resolved by Sub-agents

| Q | Answer |
|---|---|
| `axum` vs `tauri::WebviewWindow::lazy`+`LSUIElement` hidden WebView? | **axum** wins — lazy WebView still spawns WKWebView/WebView2 process (~120MB); `axum` + `tower-http ServeDir` with fallback is the only true zero-WebView. Keep `LSUIElement`/`Accessory` for tray-only. |
| Cloud sync? | **Deferred** — local-first only; JSON import/export covers backup and external sync via user tooling. |
| Native overlay worth Swift complexity? | **Yes** — only way to eliminate last WebView + `wry` leak + `tao` DPI/topmost workarounds. Cost scoped: one `swiftc` static lib mirroring two existing bridges `build.rs:386`, `softbuffer` Windows. |
| Which competitor features are trash? | See tables above — sync, playground, analytics, expander, nested folders, branch, evaluation harness, LAN/bearer extras, browser extension. Keep flat folders, tags, pin, LIKE→FTS, linear versions, `{{var}}`, copy/insert via `clipboard::paste`. |

## Sub-agent Provenance

- **Agent A (Phase 1)** — prompt library; sources `prompt_history.rs:1`, `history.rs:20`, `lib.rs:606`, `Sidebar.tsx:52`, `clipboard.rs:613`, `shortcut/mod.rs:33`.
- **Agent B (Phase 2)** — server mode; sources `lib.rs:917`, `tray.rs:228`, `portable.rs:14`, `tauri.conf.json:10`, `vite.config.ts:25`.
- **Agent C (Phase 3)** — native overlay; sources `overlay.rs:361`, `RecordingOverlay.tsx:1`, `Cargo.toml:171`, `build.rs:386`.
- **Agent D (Phase 4+5)** — FTS/spell/migrations; sources `spellcheck.rs:1`, `prompt_history.rs:54`, `settings.rs:540`, `portable.rs:60`, `cli.rs:1`, `AGENTS.md:159`.

