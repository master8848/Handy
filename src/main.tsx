import React from "react";
import ReactDOM from "react-dom/client";
import { platform } from "@tauri-apps/plugin-os";
import App from "./App";
import {
  applyTheme,
  getStoredAccent,
  getStoredTheme,
  syncThemeFromSettings,
} from "./lib/utils/theme";

// Set platform before render so CSS can scope per-platform (e.g. scrollbar styles).
// Outside a Tauri runtime (plain browser) the plugin call throws, which would
// otherwise leave a permanent white screen — fall back to a safe default.
try {
  document.documentElement.dataset.platform = platform();
} catch {
  document.documentElement.dataset.platform = "macos";
}

// Apply the last-known theme and accent synchronously before render to avoid a
// flash of the wrong palette, then reconcile with the persisted settings once
// they load.
applyTheme(getStoredTheme(), getStoredAccent());
syncThemeFromSettings();

// Initialize i18n
import "./i18n";

// Initialize model store (loads models and sets up event listeners)
import { useModelStore } from "./stores/modelStore";
useModelStore.getState().initialize();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
