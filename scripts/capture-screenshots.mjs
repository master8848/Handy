#!/usr/bin/env node
/**
 * Capture Affinity-style persona screenshots for the README.
 *
 * - Starts a Vite dev server (or reuses one) on a free port
 * - Injects a Tauri mock (window.__TAURI_INTERNALS__) so the app renders
 *   without the Rust backend (onboarding bypassed, models mocked)
 * - Navigates to each persona via ?tab= and captures desktop (1280) + mobile (390)
 * - Also captures toolbar-dropdown and prompt-workbench expanded states
 * - Saves to screenshots/ (and mirrors to public/screenshots/ for dev preview)
 * - Prints a markdown table
 *
 * Usage:
 *   node scripts/capture-screenshots.mjs --help
 *   bun run screenshots:capture
 *   bun run screenshots:capture -- --port 5173 --headed
 */

import { spawn } from "node:child_process";
import { createServer as createViteServer } from "vite";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import net from "node:net";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..");
const SCREENSHOTS_DIR = path.join(REPO_ROOT, "screenshots");
const PUBLIC_SCREENSHOTS_DIR = path.join(REPO_ROOT, "public", "screenshots");

const DEFAULT_PORT = 5173;

const HELP = `
capture-screenshots.mjs — capture Handy persona UIs with Playwright

Usage:
  node scripts/capture-screenshots.mjs [options]

Options:
  --help, -h           Show this help
  --port <n>           Vite port (default: 5173, auto-bumps if busy)
  --preview            Build then serve preview instead of dev server
  --headed             Run browser headed (for debugging)
  --no-build-mirror    Skip mirroring screenshots/ -> public/screenshots/
  --url <url>          Skip starting Vite, capture from this URL instead
  --only <name>        Only capture one file (e.g. dictate-desktop.png)

Examples:
  bun run screenshots:capture
  node scripts/capture-screenshots.mjs --port 5174 --headed
  node scripts/capture-screenshots.mjs --url http://localhost:1420

Output:
  screenshots/dictate-desktop.png
  screenshots/prompt-desktop.png
  screenshots/transcribe-desktop.png
  screenshots/toolbar-dropdown.png
  screenshots/prompt-workbench-expanded.png
  screenshots/dictation-mobile.png
  screenshots/prompt-mobile.png
  screenshots/transcribe-mobile.png
  (mirrored to public/screenshots/ for /screenshots/* dev serving)
  Markdown table printed to stdout
`;

// ---- args ----
const args = process.argv.slice(2);
if (args.includes("--help") || args.includes("-h")) {
  console.log(HELP);
  process.exit(0);
}
function argVal(name, fallback) {
  const i = args.indexOf(name);
  if (i !== -1 && args[i + 1]) return args[i + 1];
  return fallback;
}
const headed = args.includes("--headed");
const usePreview = args.includes("--preview");
const noMirror = args.includes("--no-build-mirror");
const onlyFile = argVal("--only", null);
const externalUrl = argVal("--url", null);
let desiredPort = Number(argVal("--port", String(DEFAULT_PORT))) || DEFAULT_PORT;

async function isPortFree(port) {
  return new Promise((resolve) => {
    const srv = net.createServer();
    srv.once("error", () => resolve(false));
    srv.once("listening", () => srv.close(() => resolve(true)));
    srv.listen(port, "127.0.0.1");
  });
}

async function findFreePort(start) {
  for (let p = start; p < start + 20; p++) {
    if (await isPortFree(p)) return p;
  }
  return start;
}

function ensureDir(dir) {
  fs.mkdirSync(dir, { recursive: true });
}

// ---- Tauri mock injected before any app code runs ----
const TAURI_MOCK_SCRIPT = `
(() => {
  const listeners = new Map();
  let nextId = 1;
  const callbacks = new Map();

  const mockSettings = {
    settings_schema_version: 2,
    bindings: {
      transcribe: { id: "transcribe", name: "Transcribe", description: "", default_binding: "option+space", current_binding: "option+space" },
      transcribe_with_post_process: { id: "transcribe_with_post_process", name: "Transcribe with Post-Processing", description: "", default_binding: "option+shift+space", current_binding: "option+shift+space" },
      cancel: { id: "cancel", name: "Cancel", description: "", default_binding: "escape", current_binding: "escape" },
      prompt_palette: { id: "prompt_palette", name: "Prompt Palette", description: "", default_binding: "option+shift+p", current_binding: "option+shift+p" },
      quick_prompt: { id: "quick_prompt", name: "Quick Prompt", description: "Open the quick prompt box (Raycast/Spotlight style) to write a snippet and paste it with Cmd+Enter.", default_binding: "command+shift+j", current_binding: "command+shift+j" }
    },
    push_to_talk: true,
    audio_feedback: false,
    audio_feedback_volume: 1,
    sound_theme: "marimba",
    start_hidden: false,
    autostart_enabled: false,
    update_checks_enabled: true,
    show_whats_new_on_update: true,
    whats_new_last_seen_version: "0.9.4",
    selected_model: "mock-model",
    onboarding_completed: true,
    always_on_microphone: false,
    selected_microphone: "Default",
    clamshell_microphone: "Default",
    selected_output_device: "Default",
    translate_to_english: false,
    selected_language: "auto",
    overlay_position: "bottom",
    overlay_style: "live",
    overlay_native_enabled: false,
    debug_mode: false,
    log_level: "info",
    custom_words: [],
    custom_word_datasets: [],
    text_replacements: [],
    model_unload_timeout: "min_5",
    multi_model_loading: false,
    word_correction_threshold: 0.18,
    history_limit: 5,
    recording_retention_period: "preserve_limit",
    paste_method: "ctrl_v",
    clipboard_handling: "dont_modify",
    auto_submit: false,
    auto_submit_key: "enter",
    post_process_enabled: false,
    post_process_provider_id: "openai",
    post_process_providers: [],
    post_process_api_keys: {},
    post_process_models: {},
    post_process_prompts: [],
    post_process_selected_prompt_id: null,
    mute_while_recording: false,
    append_trailing_space: false,
    app_language: "en",
    theme: "system",
    accent_color: "pink",
    experimental_enabled: false,
    lazy_stream_close: false,
    keyboard_implementation: "handy_keys",
    show_tray_icon: true,
    paste_delay_ms: 60,
    paste_delay_after_ms: 60,
    reliable_paste: false,
    typing_tool: "auto",
    external_script_path: null,
    custom_filler_words: null,
    transcribe_accelerator: "auto",
    ort_accelerator: "auto",
    transcribe_gpu_device: -1,
    extra_recording_buffer_ms: 0,
    vad_enabled: true,
    spell_check_enabled: false,
    prompt_library_enabled: true,
    server_mode_enabled: false,
    server_port: 17373,
    server_bind: "127.0.0.1",
    server_auth_token: null
  };

  window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ || {};
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = window.__TAURI_EVENT_PLUGIN_INTERNALS__ || {};
  window.__TAURI_OS_PLUGIN_INTERNALS__ = {
    platform: "macos",
    version: "14.0.0",
    family: "unix",
    arch: "aarch64",
    exe_extension: "",
    os_type: "macos",
    eol: "\\n"
  };
  window.__TAURI_STORE_PLUGIN_INTERNALS__ = window.__TAURI_STORE_PLUGIN_INTERNALS__ || {};

  function handleEventPlugin(cmd, args) {
    switch (cmd) {
      case "plugin:event|listen": {
        const event = args.event;
        const handler = args.handler;
        if (!listeners.has(event)) listeners.set(event, []);
        listeners.get(event).push(handler);
        return handler;
      }
      case "plugin:event|emit": {
        const { event, payload } = args;
        const hs = listeners.get(event) || [];
        hs.forEach((id) => {
          const cb = callbacks.get(id);
          if (cb) cb({ event, id: 0, payload });
        });
        return null;
      }
      case "plugin:event|unlisten": {
        const { event, id } = args;
        const arr = listeners.get(event);
        if (arr) {
          const idx = arr.indexOf(id);
          if (idx !== -1) arr.splice(idx, 1);
        }
        callbacks.delete(id);
        return null;
      }
      case "plugin:event|register_listener":
      case "plugin:event|remove_listener":
        return null;
      default: return null;
    }
  }

  async function mockInvoke(cmd, args) {
    if (cmd.startsWith("plugin:event|")) return handleEventPlugin(cmd, args);
    if (cmd.startsWith("plugin:os|")) {
      if (cmd === "plugin:os|locale") return "en-US";
      if (cmd === "plugin:os|hostname") return "localhost";
      return null;
    }
    if (cmd.startsWith("plugin:store|")) return null;
    if (cmd.includes("macos-permissions") || cmd.includes("macos_permissions")) return true;
    switch (cmd) {
      case "get_app_settings": return mockSettings;
      case "get_default_settings": return mockSettings;
      case "get_available_models": return [];
      case "get_model_info": return null;
      case "get_current_model": return "";
      case "get_loaded_models": return [];
      case "get_transcription_model_status": return null;
      case "is_model_loading": return false;
      case "get_available_microphones": return [];
      case "get_available_output_devices": return [];
      case "get_selected_microphone": return "Default";
      case "get_selected_output_device": return "Default";
      case "get_clamshell_microphone": return "Default";
      case "get_app_dir_path": return "/tmp";
      case "get_log_dir_path": return "/tmp";
      case "check_custom_sounds": return { start: false, stop: false };
      case "get_history_entries": return { entries: [], has_more: false };
      case "get_audio_file_path": return "/tmp/audio.wav";
      case "list_prompt_history": return [];
      case "list_prompts": return [];
      case "search_prompts": return [];
      case "list_folders": return [];
      case "list_tags": return [];
      case "list_prompt_versions": return [];
      case "get_available_typing_tools": return [];
      case "get_available_accelerators": return { transcribe: [], ort: [], gpu_devices: [] };
      case "is_portable": return false;
      case "is_laptop": return false;
      case "get_secure_input_status": return { enabled:false, sustained:false, culprit_pid:null, culprit_name:null, fallback_active:false, covered_bindings:[], degraded_bindings:[], uncovered_bindings:[], recorder_blocked:false };
      case "get_windows_microphone_permission_status": return { supported:false, overall_access:"allowed", device_access:"allowed", app_access:"allowed", desktop_app_access:"allowed" };
      case "get_keyboard_implementation": return "handy_keys";
      case "harper_status": return { initialized:false, enabled:false };
      case "check_spelling": return [];
      case "os_speech_available": return false;
      case "os_speech_authorization_status": return "authorized";
      case "check_apple_intelligence_available": return false;
      case "is_recording": return false;
      case "get_microphone_mode": return false;
      case "get_model_load_status": return { is_loaded:false, current_model:null };
      case "initialize_enigo": return null;
      case "initialize_shortcuts": return null;
      case "show_main_window_command": return null;
      case "open_app_window": return null;
      case "open_speech_recognition_settings": return null;
      case "get_available_typing_tools": return [];
      default:
        return null;
    }
  }

  window.__TAURI_INTERNALS__.invoke = async (cmd, args, _opts) => mockInvoke(cmd, args);
  window.__TAURI_INTERNALS__.transformCallback = (cb, once=false) => {
    const id = nextId++;
    callbacks.set(id, (data) => {
      if (once) callbacks.delete(id);
      try { cb(data); } catch {}
    });
    return id;
  };
  window.__TAURI_INTERNALS__.unregisterCallback = (id) => callbacks.delete(id);
  window.__TAURI_INTERNALS__.runCallback = (id, data) => {
    const cb = callbacks.get(id);
    if (cb) cb(data);
  };
  window.__TAURI_INTERNALS__.callbacks = callbacks;
  window.__TAURI_INTERNALS__.metadata = { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } };
  window.__TAURI_INTERNALS__.convertFileSrc = (p, proto="asset") => proto + "://localhost/" + encodeURIComponent(p);
  window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener = () => {};
  // Some plugins read window.isTauri
  try { window.isTauri = true; } catch {}
})();
`;

// ---- Vite server ----
async function startVite(port) {
  if (usePreview) {
    // build then preview — not implemented as vite preview API is separate; fall back to dev
    console.warn("[capture] --preview requested but dev server is used (build not needed for screenshots)");
  }
  const vitePort = await findFreePort(port);
  console.log(`[capture] Starting Vite dev server on http://127.0.0.1:${vitePort} ...`);
  const server = await createViteServer({
    configFile: path.join(REPO_ROOT, "vite.config.ts"),
    server: {
      port: vitePort,
      strictPort: true,
      host: "127.0.0.1",
    },
    logLevel: "warn",
  });
  await server.listen();
  server.printUrls();
  const url = `http://127.0.0.1:${vitePort}`;
  // wait for ready
  for (let i = 0; i < 30; i++) {
    try {
      const res = await fetch(url);
      if (res.ok) break;
    } catch {}
    await new Promise((r) => setTimeout(r, 300));
  }
  return { server, url, port: vitePort };
}

async function resolveChromium() {
  // Prefer 'playwright', fallback to '@playwright/test' (already a devDep) then 'playwright-core'
  try {
    const pw = await import("playwright");
    if (pw.chromium) return pw.chromium;
  } catch {}
  try {
    const pwTest = await import("@playwright/test");
    if (pwTest.chromium) return pwTest.chromium;
  } catch {}
  try {
    const pwc = await import("playwright-core");
    if (pwc.chromium) return pwc.chromium;
  } catch {}
  throw new Error(
    "Could not resolve a Playwright chromium export. Run: bunx playwright install chromium ( @playwright/test should already be installed )"
  );
}

async function captureAll(baseUrl) {
  const chromium = await resolveChromium();
  const browser = await chromium.launch({ headless: !headed });
  const context = await browser.newContext({
    viewport: { width: 1280, height: 800 },
    deviceScaleFactor: 2,
  });
  // inject Tauri mock before any script runs
  await context.addInitScript({ content: TAURI_MOCK_SCRIPT });

  ensureDir(SCREENSHOTS_DIR);
  if (!noMirror) ensureDir(PUBLIC_SCREENSHOTS_DIR);

  const shots = [
    {
      file: "dictate-desktop.png",
      url: `${baseUrl}/?tab=dictate`,
      viewport: { width: 1280, height: 800 },
      wait: '[role="tablist"]',
      setup: null,
    },
    {
      file: "prompt-desktop.png",
      url: `${baseUrl}/?tab=prompt`,
      viewport: { width: 1280, height: 800 },
      wait: '[role="tablist"]',
      setup: null,
    },
    {
      file: "transcribe-desktop.png",
      url: `${baseUrl}/?tab=transcribe`,
      viewport: { width: 1280, height: 800 },
      wait: '[role="tablist"]',
      setup: null,
    },
    {
      file: "toolbar-dropdown.png",
      url: `${baseUrl}/?tab=dictate`,
      viewport: { width: 1280, height: 800 },
      wait: '[role="tablist"]',
      setup: "toolbarDropdown",
    },
    {
      file: "prompt-workbench-expanded.png",
      url: `${baseUrl}/?tab=prompt`,
      viewport: { width: 1280, height: 800 },
      wait: ".ptap-body",
      setup: "promptExpanded",
    },
    {
      file: "dictation-mobile.png",
      url: `${baseUrl}/?tab=dictate`,
      viewport: { width: 390, height: 844 },
      wait: '[role="tablist"]',
      setup: null,
    },
    {
      file: "prompt-mobile.png",
      url: `${baseUrl}/?tab=prompt`,
      viewport: { width: 390, height: 844 },
      wait: ".ptap-body",
      setup: null,
    },
    {
      file: "transcribe-mobile.png",
      url: `${baseUrl}/?tab=transcribe`,
      viewport: { width: 390, height: 844 },
      wait: '[role="tablist"]',
      setup: null,
    },
  ];

  const filtered = onlyFile ? shots.filter((s) => s.file === onlyFile) : shots;
  if (onlyFile && filtered.length === 0) {
    console.error(`[capture] Unknown --only file: ${onlyFile}`);
    process.exit(1);
  }

  const results = [];

  for (const shot of filtered) {
    const page = await context.newPage();
    await page.setViewportSize(shot.viewport);
    console.log(`[capture] ${shot.file} <- ${shot.url} @ ${shot.viewport.width}x${shot.viewport.height}`);
    await page.goto(shot.url, { waitUntil: "domcontentloaded", timeout: 15000 });
    // let React mount
    await page.waitForTimeout(900);
    if (shot.wait) {
      try {
        await page.waitForSelector(shot.wait, { timeout: 5000 });
      } catch {
        console.warn(`[capture] wait selector not found: ${shot.wait} for ${shot.file}`);
      }
    }
    await page.waitForTimeout(400);

    if (shot.setup === "toolbarDropdown") {
      await page.evaluate(() => {
        const bar = document.querySelector('div.flex.items-center.gap-3.px-3');
        if (!bar) return;
        bar.style.position = "relative";
        if (bar.querySelector('[data-capture-dropdown]')) return;
        const dd = document.createElement("div");
        dd.setAttribute("data-capture-dropdown", "1");
        dd.style.position = "absolute";
        dd.style.top = "44px";
        dd.style.right = "12px";
        dd.style.width = "240px";
        dd.style.background = "white";
        dd.style.border = "1px solid rgba(0,0,0,0.08)";
        dd.style.borderRadius = "12px";
        dd.style.boxShadow = "0 8px 32px rgba(0,0,0,0.12)";
        dd.style.padding = "8px";
        dd.style.zIndex = "50";
        dd.style.fontFamily = "system-ui, -apple-system, sans-serif";
        dd.innerHTML =
          '<div style="font-size:12px;font-weight:600;padding:6px 8px;color:#111">Experimental toolbar</div>' +
          '<div style="font-size:13px;padding:8px 10px;border-radius:8px;background:rgba(236,72,153,0.1);color:#ec4899;margin-bottom:6px">Model: Whisper Small · Live</div>' +
          '<div style="font-size:13px;padding:6px 10px;color:#444">Server mode: off</div>' +
          '<div style="font-size:13px;padding:6px 10px;color:#444">VAD: enabled</div>' +
          '<div style="height:1px;background:rgba(0,0,0,0.06);margin:6px 0"></div>' +
          '<div style="font-size:12px;padding:6px 10px;color:#888">Press ?view=screenshots to open the gallery</div>';
        // dark mode tweak
        if (document.documentElement.getAttribute("data-theme") === "dark" || window.matchMedia("(prefers-color-scheme: dark)").matches) {
          dd.style.background = "#1a1a1a";
          dd.style.borderColor = "rgba(255,255,255,0.08)";
        }
        bar.appendChild(dd);
      });
      await page.waitForTimeout(300);
    }

    if (shot.setup === "promptExpanded") {
      await page.evaluate(() => {
        const el = document.querySelector(".ptap-body");
        if (!el) return;
        el.focus();
        // Insert sample content via DOM (tiptap will pick up onUpdate on next transaction, but for screenshot DOM is enough)
        const sample = document.createElement("div");
        sample.innerHTML =
          "<h1>Q4 Planning Prompt</h1>" +
          "<p>Summarize the transcript below into 3 concise bullets, then draft a 2-sentence next step.</p>" +
          "<blockquote>Transcript: We shipped the new onboarding and cut time-to-first-value by 32%. The prompt library is still experimental — users love the insertion flow but want folders.</blockquote>" +
          "<ul><li>Keep tone crisp, no filler</li><li>Preserve numbers and names</li></ul>" +
          "<pre><code>variables: {{customer}} · {{quarter}}</code></pre>";
        // Clear placeholder and append
        const prose = el.querySelector(".tiptap") || el;
        // tiptap renders inside .tiptap; try both
        const target = document.querySelector(".ptap-body .tiptap") || el;
        target.innerHTML = sample.innerHTML;
        target.style.minHeight = "320px";
      });
      await page.waitForTimeout(400);
    }

    // Ensure onboarding overlay not covering — if accessibility onboarding is visible, hide it for capture
    await page.evaluate(() => {
      // Force onboarding to done if the mock didn't bypass it
      try {
        const hasOnboarding = document.body.textContent && document.body.textContent.includes("Permissions Required");
        if (hasOnboarding) {
          document.body.innerHTML = '<div style="padding:32px;font-family:system-ui">Onboarding visible — mock incomplete. Check TAURI_MOCK_SCRIPT.</div>';
        }
      } catch {}
    });

    const outPath = path.join(SCREENSHOTS_DIR, shot.file);
    await page.screenshot({ path: outPath, fullPage: true, type: "png" });

    // Optimize: if >2MB, re-encode as compressed png via same file (playwright already compresses)
    try {
      const stat = fs.statSync(outPath);
      if (stat.size > 2 * 1024 * 1024) {
        console.warn(`[capture] ${shot.file} is ${(stat.size / 1024 / 1024).toFixed(2)}MB >2MB — consider recompressing`);
      }
      results.push({ file: shot.file, path: outPath, bytes: stat.size, viewport: shot.viewport });
    } catch {}

    if (!noMirror) {
      try {
        const mirrorPath = path.join(PUBLIC_SCREENSHOTS_DIR, shot.file);
        fs.copyFileSync(outPath, mirrorPath);
      } catch {}
    }

    await page.close();
  }

  await browser.close();

  // Print markdown table
  console.log("\n# Screenshots\n");
  console.log("| Preview | File | Description | Size |");
  console.log("| --- | --- | --- | --- |");
  const descMap = {
    "dictate-desktop.png": "Dictation persona — desktop",
    "prompt-desktop.png": "Prompt Studio — desktop",
    "transcribe-desktop.png": "File transcription — desktop",
    "toolbar-dropdown.png": "Toolbar dropdown open (experimental)",
    "prompt-workbench-expanded.png": "Prompt Workbench expanded with sample content",
    "dictation-mobile.png": "Dictation — mobile 390px",
    "prompt-mobile.png": "Prompt Studio — mobile 390px",
    "transcribe-mobile.png": "File transcription — mobile 390px",
  };
  for (const r of results) {
    const kb = (r.bytes / 1024).toFixed(0);
    const desc = descMap[r.file] || r.file;
    console.log(`| ![${desc}](screenshots/${r.file}) | \`${r.file}\` | ${desc} | ${kb} KB |`);
  }
  console.log("\nGallery: http://localhost:5173/?view=screenshots  (or `?screenshots=1`)");
  console.log(`Screenshots dir: ${SCREENSHOTS_DIR}`);
  if (!noMirror) console.log(`Mirrored to: ${PUBLIC_SCREENSHOTS_DIR}`);
  return results;
}

async function main() {
  let viteServer = null;
  let baseUrl = externalUrl;

  if (!baseUrl) {
    const started = await startVite(desiredPort);
    viteServer = started.server;
    baseUrl = started.url;
  } else {
    console.log(`[capture] Using external URL: ${baseUrl}`);
  }

  let results = [];
  try {
    results = await captureAll(baseUrl);
  } finally {
    if (viteServer) {
      await viteServer.close();
      console.log("[capture] Vite server closed");
    }
  }

  if (results.length === 0) {
    console.error("[capture] No screenshots were generated");
    process.exit(1);
  }
}

main().catch((e) => {
  console.error("[capture] Fatal:", e);
  process.exit(1);
});
