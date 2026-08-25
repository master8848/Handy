/**
 * Shared debounce intervals.
 *
 * - `SEARCH_DEBOUNCE_MS` (150 ms) — FTS-backed prompt search (`prompt_fts`
 *   `MATCH` + `bm25`) feels instant (<15 ms for 5k) but still gates IPC so
 *   fast typing doesn't flood the Rust side. Do NOT lower to 0 ms.
 * - `SPELL_CHECK_DEBOUNCE_MS` (300 ms) — offline Harper linting
 *   (`spellcheck.rs` via `spawn_blocking`). Longer window avoids per-chunk
 *   churn mid-speech and matches the editor + native overlay contract.
 *
 * Native overlay spec (Phase 3/4, doc-only until native panel lands):
 * - Streaming Live panel is a non-activating `NSPanel`/`HWND` — no inline
 *   suggestion popover inside it (transient 64 px wavy underline for ~2 s,
 *   not actionable). Spell stays in the TipTap editor (Harper).
 * - If the native overlay ever does local spell, it should mirror:
 *   macOS `NSSpellChecker.checkSpellingOfString:startingAt:` (sync, no IPC)
 *   with 300 ms debounce + `checkSeqRef` invalidation; Windows defers
 *   indefinitely (`ISpellCheckerFactory` COM cost). Editor stays
 *   `harper-core` regardless — `spellCheckExtension.ts:56` is the source of
 *   truth.
 * - Rendering (when needed): Swift `AttributedString` wavy underline
 *   `NSUnderlineStyle.thick` + `NSUnderlinePattern.dash` in
 *   `var(--color-error)`, WinUI `TextDecoration` via `SpellingIssue[]` events;
 *   keep Harper as the single linter for now (IPC fine for v1).
 */
export const SEARCH_DEBOUNCE_MS = 150;
export const SPELL_CHECK_DEBOUNCE_MS = 300;
export const PROMPT_HISTORY_DEBOUNCE_MS = 300;
export const PROMPT_HISTORY_DEBOUNCE = PROMPT_HISTORY_DEBOUNCE_MS;
