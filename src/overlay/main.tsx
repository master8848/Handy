import React from "react";
import ReactDOM from "react-dom/client";
import RecordingOverlay from "./RecordingOverlay";
import { applyTheme, getStoredAccent, getStoredTheme } from "@/lib/utils/theme";
import "@/i18n";

// The overlay window is a separate entry point, so the main app's boot-time
// theming never runs here. Apply the stored theme and accent synchronously
// before render so the pill matches the app palette (including a forced
// light/dark theme the OS wouldn't otherwise honor).
applyTheme(getStoredTheme(), getStoredAccent());

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <RecordingOverlay />
  </React.StrictMode>,
);
