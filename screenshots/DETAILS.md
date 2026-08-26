# Screenshots — Details Catalog

Date: 2026-08-26
Source of truth: `scripts/capture-screenshots.mjs` (10 shots), `src/App.tsx` routing, `src/components/quick-prompt/QuickPromptBox.tsx`, `src/components/screenshots/ScreenshotsPage.tsx`, `src/components/prompt-workbench/PromptWorkbench.tsx`.

Regenerate:

```bash
bun run screenshots:capture
# or
node scripts/capture-screenshots.mjs
node scripts/capture-screenshots.mjs --headed --port 5174
node scripts/capture-screenshots.mjs --url http://localhost:1420
node scripts/capture-screenshots.mjs --only dictate-desktop.png
```

Vite dev server starts on `127.0.0.1:5173` (auto-bumps if busy), injects `window.__TAURI_INTERNALS__` mock (bypasses Rust/Tauri, `onboarding_completed: true`, empty models/prompts/history), navigates with Playwright `chromium`, `deviceScaleFactor: 2`, `screenshot({ fullPage: true })`, writes to `screenshots/*.png` and mirrors to `public/screenshots/*.png` (for `GET /screenshots/*` in dev). Gallery route is `?view=screenshots` (alias `?screenshots=1`).

On disk 2026-08-26: 8 files present (8 mirrored). 2 capture-defined shots not yet materialized: `quick-prompt-desktop.png` (`?view=quick-prompt`) and `settings-general-desktop.png` (`?view=settings`).

| Preview | File | Viewport (logical) | Physical (×2) | URL | What it shows |
| --- | --- | --- | --- | --- | --- |
| ![Dictate — Desktop](dictate-desktop.png) | `dictate-desktop.png` | 1280×800 | 2560×1600 | `/?tab=dictate` | Dictate persona · TopTabBar (Dictate/Prompt/Transcribe) · MainSidebar (Dictate, Prompt Studio, Library, History, Transcribe) · Home prompt composer (Tiptap, `home.placeholder`) · mic button (idle) · history strip · Footer |
| ![Prompt — Desktop](prompt-desktop.png) | `prompt-desktop.png` | 1280×800 | 2560×1600 | `/?tab=prompt` | Prompt Studio persona · Composer card with `PromptFormatToolbar` · Send bar (Mic/Copy/Clear/Send) · Library canvas (search, folder pills, tag pills, 6-card grid) · Recent inserts strip · Tiptap placeholder `promptStudio.composerPlaceholder` |
| ![Transcribe — Desktop](transcribe-desktop.png) | `transcribe-desktop.png` | 1280×800 | 2560×1600 | `/?tab=transcribe` | Transcribe persona · Dashed drop zone (`FileAudio` icon) · `transcribe.dropTitle` / `transcribe.dropHint` (MP3/WAV/M4A/OGG/FLAC) · `transcribe.browseFiles` · Phase area idle |
| ![Toolbar dropdown](toolbar-dropdown.png) | `toolbar-dropdown.png` | 1280×800 | 2560×1600 | `/?tab=dictate` + injected `toolbarDropdown` | Same as dictate-desktop plus synthetic absolute dropdown (injected via `page.evaluate`) on the top bar: "Experimental toolbar" · Model: Whisper Small · Live · Server mode: off · VAD: enabled · footer hint |
| ![Prompt Workbench Expanded](prompt-workbench-expanded.png) | `prompt-workbench-expanded.png` | 1280×800 | 2560×1600 | `/?tab=prompt` + injected `promptExpanded` | Prompt Studio with composer pre-filled via DOM injection: `Q4 Planning Prompt` h1, summary instruction, blockquote transcript, bullets, `variables: {{customer}} · {{quarter}}` code block · library grid underneath |
| ![Dictation — Mobile](dictation-mobile.png) | `dictation-mobile.png` | 390×844 | 780×1688 (file 898×1688 due to fullPage + scrollbar) | `/?tab=dictate` | Dictate persona at mobile width — tab bar stacked, sidebar hidden (`sm:flex`), composer full-width, history strip collapsed layout |
| ![Prompt — Mobile](prompt-mobile.png) | `prompt-mobile.png` | 390×844 | 780×1688 (file 898×1688) | `/?tab=prompt` | Prompt Studio at mobile — composer, single-column library grid (`grid-cols-1 sm:grid-cols-2`), search full-width, folder pills scrollable |
| ![Transcribe — Mobile](transcribe-mobile.png) | `transcribe-mobile.png` | 390×844 | 780×1688 (file 898×1688) | `/?tab=transcribe` | Transcribe drop zone at mobile width, centered layout |
| _(not yet on disk)_ | `quick-prompt-desktop.png` | 800×600 | 1600×1200 | `/?view=quick-prompt` | Quick Prompt spotlight (Raycast/Spotlight style) — centered 640px card (`quick-prompt-root`) · drag header · textarea (`quickPrompt.placeholder` with `{{meta}}`) · footer hints `⌘↵ paste / Esc close / ↵ new line` · Cancel + Paste buttons · injected mock history list when capture runs |
| _(not yet on disk)_ | `settings-general-desktop.png` | 1280×800 | 2560×1600 | `/?view=settings` | Auxiliary settings window (`getWindowView` → `settings`) · `Sidebar` (`WINDOW_SECTIONS['settings']`) · General section (shortcuts, push-to-talk, language) · Footer |

Disk sizes 2026-08-26 (`ls -lh`, `file`):

| File | Size | `file` dimensions |
| --- | --- | --- |
| `dictate-desktop.png` | 114K | 2560×1600 8-bit/color RGB |
| `prompt-desktop.png` | 97K | 2560×1600 |
| `transcribe-desktop.png` | 74K | 2560×1600 |
| `toolbar-dropdown.png` | 144K | 2560×1600 |
| `prompt-workbench-expanded.png` | 155K | 2560×1600 |
| `dictation-mobile.png` | 105K | 898×1688 |
| `prompt-mobile.png` | 81K | 898×1688 |
| `transcribe-mobile.png` | 62K | 898×1688 |
| `quick-prompt-desktop.png` | — (not generated yet) | — |
| `settings-general-desktop.png` | — (not generated yet) | — |

Mirrored 1:1 in `public/screenshots/` (same 8 files, same sizes).

---

## Routing (src/App.tsx:46-60, :84-142)

- Main window = no `?view=` param. Uses `TopTabBar` (`MainTab`: `dictate` | `prompt` | `transcribe`) + `MainSidebar` (`MainNavId`: `dictate` | `prompt` | `library` | `history` | `transcribe` | `settings`). `?tab=` selects persona (also persisted to `localStorage handy.activeTab` / `handy.mainNav`). `?nav=` selects sidebar nav.
- Auxiliary windows: `?view=settings` → settings sidebar window; `?view=studio` → prompt-library studio window; `?view=quick-prompt` → `QuickPromptBox` spotlight (`isQuickPromptView`); `?view=screenshots` or `?screenshots=1` → `ScreenshotsPage` (bypasses onboarding, `src/App.tsx:439`). Screenshots view is dev-only intent, docs URL `http://localhost:5173/?view=screenshots`.

Capture waits/selectors (`scripts/capture-screenshots.mjs:383-454`): `[role="tablist"]` for dictate/transcribe desktops, `.ptap-body` for prompt desktop/expanded/mobile, `textarea` for quick-prompt, `[role="tablist"], nav` for settings. Each shot does `goto(domcontentloaded)` + 900ms React mount + `waitForSelector` (5s) + 400ms, then optional setup injection.

`deviceScaleFactor: 2` on the Playwright context (`src/App.tsx` — actually `scripts/capture-screenshots.mjs:375`) means logical 1280×800 renders at 2560×1600 physical; 390×844 at 780×1688 (observed 898 width due to `fullPage` + scrollbar chrome — treat as ~780 logical doubled).

---

## Per-file expanded

### 1. `dictate-desktop.png` — `1280×800` — `/?tab=dictate` — wait `[role="tablist"]`
- **Purpose:** Primary persona — dictation landing for README hero.
- **Key UI elements:** `TopTabBar` (Mic/NotebookPen/FileAudio icons, active `Dictate`), `MainSidebar` (badge counts for Library/History), central `Home` composer (`src/components/settings/home` / `Home` — Tiptap editor, mic idle, elapsed timer hidden, error slot, Paste/Clear row), `SecureInputWarning` / `AccessibilityPermissions` banners, `Footer` (update + harper status), `PromptPalette` global.
- **Mocks/notes:** Tauri mock returns empty history/models; editor placeholder from `home.placeholder` ("Write your prompt here — or dictate it…").
- **Related i18n:** `tabs.dictate`, `sidebar.*`, `home.*`, `dictation.*`, `footer.*`.
- **Extend:** Add onboarding overlay or recording/transcribing phase by toggling `phase` state in `PromptWorkbench`/`Home`.

### 2. `prompt-desktop.png` — `1280×800` — `/?tab=prompt` — wait `[role="tablist"]`
- **Purpose:** Prompt Studio persona overview.
- **Key UI elements:** Composer card (`PromptFormatToolbar` + Tiptap `.ptap-workbench !min-h-[160px]` with `promptStudio.composerPlaceholder`), Send bar (Mic primary/danger, Copy/Clear/Send), Library canvas (search → `promptStudio.searchPlaceholder`, folder pills `promptStudio.allFolders`, tag pills `promptStudio.allTags`, `StudioPromptCard` grid 6 items, `+ New` button, `Show all/Show less` controls), History strip (Recent inserts, 3 items), spell-check toggle, error slot.
- **Mocks/notes:** `list_prompts`, `list_folders`, `list_tags` all `[]`; grid empty state "No prompts yet".
- **Related i18n:** `promptStudio.*`, `promptLibrary.*`, `tabs.prompt`.
- **Extend:** Populate mock with a prompt array to show cards with tags/folders/variables.

### 3. `transcribe-desktop.png` — `1280×800` — `/?tab=transcribe` — wait `[role="tablist"]`
- **Purpose:** File transcription persona.
- **Key UI elements:** Dashed drop zone card, `FileAudio` icon, `transcribe.dropTitle`, `transcribe.dropHint` (formats MP3/WAV/M4A/OGG/FLAC), `transcribe.browseFiles` link, phase row (Loader2 + fileName + modelName when active), error slot (`transcribe.tryAgain`), result block (`FileText` + `transcribe.result` + Copy/Copied + readonly textarea).
- **Mocks/notes:** Idle state captured; no file selected. Mock reports no available models except `mock-model`.
- **Related i18n:** `transcribe.*`, `sidebar.transcribe`.
- **Extend:** Trigger `transcribeAudioFile` mock to show `decoding`/`loading_model`/`finalizing` phases.

### 4. `toolbar-dropdown.png` — `1280×800` — `/?tab=dictate` — wait `[role="tablist"]` + `setup: toolbarDropdown`
- **Purpose:** Experimental toolbar dropdown (no real component — synthetic overlay for docs).
- **Key UI elements:** All of dictate-desktop plus injected `div[data-capture-dropdown]` absolute at `top:44px right:12px 240px` with: header "Experimental toolbar" (12px/600), row "Model: Whisper Small · Live" (pink tint), rows "Server mode: off", "VAD: enabled", divider, hint "Press ?view=screenshots…". Targets `div.flex.items-center.gap-3.px-3` (TopTabBar bar); sets `position:relative` before append. Dark mode branch (`data-theme=dark` or `prefers-color-scheme: dark`) → `bg:#1a1a1a`.
- **Mocks/notes:** Pure DOM injection in `capture-screenshots.mjs:480-514`; no Tauri/real toolbar code exercised. Remove or replace when a real toolbar dropdown lands.
- **Related i18n:** `settings.advanced.toolbar.*`, `screenshots.items.toolbarDropdown.*`.
- **Extend:** Edit the injected `innerHTML` to reflect real toolbar controls once implemented.

### 5. `prompt-workbench-expanded.png` — `1280×800` — `/?tab=prompt` — wait `.ptap-body` + `setup: promptExpanded`
- **Purpose:** Prompt Studio with filled composer (docs/demo content).
- **Key UI elements:** Composer body replaced via `querySelector .ptap-body .tiptap || .ptap-body` innerHTML: `<h1>Q4 Planning Prompt</h1>`, instruction paragraph, `<blockquote>` transcript, `<ul><li>` bullets, `<pre><code>variables: {{customer}} · {{quarter}}</code></pre>`; `minHeight:320px` to avoid collapsed look. Library/history underneath unchanged.
- **Mocks/notes:** Injected post-mount in `capture-screenshots.mjs:516-536`; not a Tiptap transaction — sufficient for static screenshot. Tiptap placeholder hidden after injection.
- **Related i18n:** `promptStudio.*`.
- **Extend:** Replace `sample.innerHTML` in the capture script to showcase different prompt patterns; for variable highlighting test, add `{{variable}}` chips via `VariableChipExtension`.

### 6. `dictation-mobile.png` — `390×844` — `/?tab=dictate` — wait `[role="tablist"]`
- **Purpose:** Mobile viewport for dictate persona (responsive check).
- **Key UI elements:** Same content as dictate-desktop but `MainSidebar` hidden (`hidden sm:flex`), `TopTabBar` remains, composer spans width, padding `p-4` preserved, footer stacked.
- **Mocks/notes:** Same mock as desktop; screenshot helper omits name `dictation-mobile` (note extra `ion`) — `SCREENSHOT_LIST` entry `dictateMobile` → file `dictation-mobile.png` (historic name).
- **Related i18n:** Same as dictate-desktop.
- **Extend:** Capture at 375×812 or 414×896 by editing `viewport` in capture script; deviceScaleFactor stays 2.

### 7. `prompt-mobile.png` — `390×844` — `/?tab=prompt` — wait `.ptap-body`
- **Purpose:** Prompt Studio mobile.
- **Key UI elements:** Composer toolbar wraps (hidden spell-check label on small), single-column grid (`grid-cols-1`), folder pills `overflow-x-auto scrollbar-none`, tag pills wrap, history strip full-width.
- **Mocks/notes:** Same mock; `.ptap-body` wait ensures Tiptap mounted even on mobile.
- **Related i18n:** `promptStudio.*`.
- **Extend:** Test collapsed library/history toggles — they render correctly on mobile via `setLibraryExpanded` / `setHistoryCollapsed`.

### 8. `transcribe-mobile.png` — `390×844` — `/?tab=transcribe` — wait `[role="tablist"]`
- **Purpose:** File transcription mobile.
- **Key UI elements:** Drop zone centered, `max-w-3xl` still applies but card is full-width within viewport padding, icons scaled, text wraps.
- **Mocks/notes:** Idle state.
- **Related i18n:** `transcribe.*`.
- **Extend:** Same as desktop variant.

### 9. `quick-prompt-desktop.png` — `800×600` — `/?view=quick-prompt` — wait `textarea` + `setup: quickPrompt`
- **Purpose:** Quick Prompt spotlight window (`QuickPromptBox.tsx:17`). Flagged as `quick-prompt-desktop.png` in capture, not yet in `SCREENSHOT_LIST` gallery.
- **Key UI elements:** Full-screen `quick-prompt-root` flex center `p-4`, card `max-w-[640px] rounded-[16px] shadow-[0_16px_48px]` with: drag header (`data-tauri-drag-region`, `bg-logo-primary` pulse dot, "Quick Prompt", close `×` button), textarea `min-h-[96px] max-h-[160px] rounded-xl bg-mid-gray/[0.06]` with placeholder `quickPrompt.placeholder` ("Write a prompt or snippet… (Enter for new line, {{meta}}+Enter to paste)" — `meta` = `⌘` on Mac else `Ctrl`), history suggestions (6 max, `.line-clamp-2`, timestamp via `toLocaleDateString`, keyboard hints "↑↓ navigate · Tab fill · Double-click paste"), footer hints `⌘↵ paste / Esc close / ↵ new line` + Cancel + "Paste ⌘↵" (disabled until `text.trim()`; `Pasting…` while `quickPromptPaste` pending). `onQuickPromptPaste` uses `quickPromptPaste` + `listPromptHistory`.
- **Mocks/notes:** Capture injects via `page.evaluate`: sets textarea value to "Summarize the standup notes into 3 bullets… Transcript: shipped onboarding, cut time-to-value by 32%…", fires `input`/`change` with native `HTMLTextAreaElement.value` setter to trigger React state, injects `[data-capture-history]` fake history list (3 rows: Today/Yesterday/2h ago) — supports dark/light via inline rgba. `ScreenshotsPage` does not yet list this file; gallery shows missing placeholder if added without mirroring.
- **Related i18n:** `quickPrompt.placeholder`, `quickPrompt.title`, `promptStudio.sendFailed`, `promptStudio.copied`, `common.loading`.
- **Extend:** Edit the injected `ta.value` string in `capture-screenshots.mjs:540-573` to showcase a different prompt; adjust `viewport: 800×600` to test 640×480 minimal window.

### 10. `settings-general-desktop.png` — `1280×800` — `/?view=settings` — wait `[role="tablist"], nav`
- **Purpose:** Auxiliary settings window (separate WebviewWindow, not main window).
- **Key UI elements:** `src/App.tsx:46 getWindowView` → `settings` window path (`isMainWindow=false`, `onboardingStep="done"`), layout `Sidebar` + scrollable content + `Footer`, `WINDOW_SECTIONS['settings']` (General/Appearance/Models/Advanced/Post Process/History/Debug/About/Transcribe), initial section `general` showing `Home` shortcuts (bindings `transcribe`/`transcribe_with_post_process`/`cancel`/`quick_prompt`/`prompt_palette`), `pushToTalk`, `Language` picker. TopTabBar/MainSidebar not rendered here.
- **Mocks/notes:** Tauri mock `bindings` + `selected_model: "mock-model"` + `prompt_library_enabled: true` etc.; no real settings persistence needed. Screenshot waits for `[role="tablist"], nav` to catch either the sidebar tabs or nav list before capture.
- **Related i18n:** `settings.general.*`, `sidebar.*`, `theme.*`, `appLanguage.*`.
- **Extend:** Change `?view=settings` to `?view=studio` to capture Prompt Library studio window instead; or add `?view=settings&section=advanced` (future) if section deep-linking landed.

---

## Tauri mock (capture-only, `scripts/capture-screenshots.mjs:105-317`)

- Bypasses onboarding: `onboarding_completed: true`, `platform: "macos"`, `locale: "en-US"`; stubs `get_app_settings`, `get_available_models: []`, `list_prompts/search_prompts/list_folders/list_tags/list_prompt_versions: []`, `list_prompt_history: []`, `get_history_entries: {entries:[], has_more:false}`, `is_recording: false`, `prompt_library_enabled: true`, etc.
- Suppresses onboarding overlay if `Permissions Required` text detected (`page.evaluate` hide).
- Playwright context `addInitScript` runs before app code.

## How to extend / add a shot

1. Add entry to `shots[]` in `scripts/capture-screenshots.mjs:383-454` (`file`, `url`, `viewport`, `wait`, `setup`). Add handler in the `if (shot.setup === "...")` blocks for DOM injection.
2. Optionally add `SCREENSHOT_LIST` entry in `src/components/screenshots/ScreenshotsPage.tsx:13-62` (requires `titleKey`/`descriptionKey` in `src/i18n/locales/en/translation.json:1027 "screenshots.items.*"`).
3. Run `bun run screenshots:capture -- --only <file>` to iterate, then full `bun run screenshots:capture`.
4. Commit `screenshots/*.png` (source) — `public/screenshots/` is auto-mirrored for dev serving; CI may verify sizes.
5. `npm run check:translations` ensures new i18n keys are present in all locales.

## See also

- `screenshots/README.md` (generated gallery table if present)
- `public/screenshots/` — dev-served mirror of this folder
- `vite.config.ts` — dev server config used by capture
