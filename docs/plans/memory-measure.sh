#!/usr/bin/env bash
# memory-measure.sh — Phase 5 QA harness: measure Handy memory wins.
#
# Uses:
#  - `ps -o rss` for process RSS (WKWebView / WebView2 / WebKitGTK + Handy)
#  - `app.webview_windows().keys()` count from Handy's log (lib.rs:121
#    `log::error!("Main window not found. Webview labels: {:?}", ...)`) and
#    from the debug panel's live log stream. Set `RUST_LOG=info` to see it.
#
# Usage:
#   chmod +x docs/plans/memory-measure.sh
#   ./docs/plans/memory-measure.sh
#   ./docs/plans/memory-measure.sh --pid 12345
#
# Expected:
#   Server mode on  → no `main` WebView (0 listed)  → saves ~150–300 MB
#   Native overlay on → no `recording_overlay` WebView → saves ~30–80 MB

set -euo pipefail

PID="${1:-}"
if [[ "$PID" == --pid ]]; then PID="${2:-}"; fi

find_pids() {
  # Handy binary name varies: handy / Handy / handy_app_lib
  pgrep -fl "Handy|handy" || true
}

if [[ -z "$PID" ]]; then
  echo "=== Handy processes (pgrep) ==="
  find_pids | sed 's/^/  /' || echo "  (none found)"
  echo
  echo "Tip: pass --pid <PID> to measure a specific Handy PID."
  echo
  # Measure all Handy pids
  PIDS=$(pgrep -f "Handy|handy" | tr '\n' ' ' || true)
  if [[ -z "$PIDS" ]]; then
    echo "No Handy process found. Start Handy first (bun run tauri dev / cargo run / installed app)."
    echo
    echo "=== Manual memory checks (when Handy is running) ==="
  else
    echo "=== RSS per process (ps -o pid,rss,command) ==="
    # Header
    ps -o pid=,rss=,command= -p $(echo "$PIDS" | tr ' ' ',' | sed 's/,$//') 2>/dev/null || ps -o pid,rss,command | head -n 20
    echo
    # Platform-specific helpers
    if [[ "$(uname)" == "Darwin" ]]; then
      echo "=== macOS: WebView renderer (WKWebView) is in-process, check Activity Monitor ==="
      echo "  Activity Monitor → Memory tab → Handy → Memory column (should drop ~150-300 MB with server mode)"
      echo "  Console.app: filter handy for 'Webview labels:' (lib.rs:121) to see window list"
    elif [[ -f /proc/self/status ]]; then
      echo "=== Linux: WebView is in-process (WebKitGTK), check RSS above ==="
    else
      echo "=== Windows: check Task Manager → Details → Working Set for Handy + msedgewebview2.exe ==="
    fi
    echo
  fi
else
  echo "=== RSS for PID $PID ==="
  ps -o pid,rss,vsz,command -p "$PID" 2>/dev/null || ps -o pid,rss,command | grep "$PID"
  echo
fi

echo "=== Manual verification checklist (Phase 5 QA matrix) ==="
cat <<'CHECK'
1. Server mode: Settings → Advanced → Server Mode → enable → Restart
   - Expect log: "Server mode enabled — skipping main WebviewWindow" (lib.rs)
   - Expect log: "Handy server listening on 127.0.0.1:PORT" (server.rs)
   - ps RSS should drop ~150–300 MB; browser http://127.0.0.1:PORT/manage works
   - Tray → Open in Browser / Open Settings Window (lazy WebviewWindowBuilder) works
   - app.webview_windows().keys() logged on missing main == [] or [recording_overlay]

2. Native overlay: Settings → Advanced → Overlay Native (macOS/Windows, default true) → enable
   - No `recording_overlay` WebView in window list; `NSPanel` (macOS) / `HWND` layered window (Windows) visible
   - Record → mic levels update at 60fps (no IPC lag), waveform 9 bars, timer/cancel hit-test

3. Search: import 5k prompts via Prompt Library → Import JSON → search "hello" 10×
   - p50 <15ms FTS (bm25), fallback LIKE <50ms on syntax error; `ps -o rss` stable (~+5 MB FTS index)

4. Portable: drop `portable` marker + Data/ dir next to exe, toggle server mode
   - All files under Data/ (settings_store.json, prompt_library.db, prompt_history.db); no %APPDATA% write

5. Second instance: while Handy is running, `handy --toggle-transcription` / `--cancel` / plain launch
   - Plain launch with server mode → opens browser, not a dead show_main_window; toggles forward via signal_handle
CHECK

echo
echo "=== End ==="
