import { commands, type AccentColor, type Theme } from "@/bindings";

/**
 * Appearance theme handling.
 *
 * Handy already ships a full light palette and a full dark palette (see
 * `App.css`). This module lets the user pick which one is used instead of
 * always following the OS:
 *  - `system` removes the override so the `prefers-color-scheme` media query
 *    governs (the historical behaviour).
 *  - `light` / `dark` set `data-theme` on the document root, whose
 *    higher-specificity CSS selectors win over the media query.
 *
 * The choice is persisted in `AppSettings` (source of truth) and mirrored to
 * localStorage so it can be applied synchronously on boot, before React mounts,
 * avoiding a flash of the wrong palette.
 */

export const THEME_STORAGE_KEY = "handy.theme";
export const ACCENT_STORAGE_KEY = "handy.accent";

export const THEME_OPTIONS: Theme[] = ["system", "light", "dark"];

export const ACCENT_OPTIONS: AccentColor[] = [
  "pink",
  "orange",
  "purple",
  "green",
  "blue",
  "red",
];

const isTheme = (value: unknown): value is Theme =>
  value === "system" || value === "light" || value === "dark";

const isAccentColor = (value: unknown): value is AccentColor =>
  ACCENT_OPTIONS.includes(value as AccentColor);

/**
 * Apply a theme and accent color to the document root and remember them for
 * the next launch. The accent maps to `data-accent`, whose selectors in
 * `theme.css` recolor the logo/UI tokens per the chosen highlight color.
 */
export const applyTheme = (
  theme: Theme,
  accent: AccentColor = "pink",
): void => {
  const root = document.documentElement;
  if (theme === "system") {
    delete root.dataset.theme;
  } else {
    root.dataset.theme = theme;
  }
  root.dataset.accent = accent;
  try {
    localStorage.setItem(THEME_STORAGE_KEY, theme);
    localStorage.setItem(ACCENT_STORAGE_KEY, accent);
  } catch {
    // localStorage may be unavailable (e.g. private mode); the setting still
    // persists in AppSettings, so this only costs a one-frame flash on boot.
  }
};

/** Read the last-applied theme for synchronous boot-time application. */
export const getStoredTheme = (): Theme => {
  try {
    const stored = localStorage.getItem(THEME_STORAGE_KEY);
    if (isTheme(stored)) return stored;
  } catch {
    // ignore
  }
  return "system";
};

/** Read the last-applied accent color for synchronous boot-time application. */
export const getStoredAccent = (): AccentColor => {
  try {
    const stored = localStorage.getItem(ACCENT_STORAGE_KEY);
    if (isAccentColor(stored)) return stored;
  } catch {
    // ignore
  }
  return "pink";
};

/** Apply the persisted theme and accent from AppSettings (the source of truth). */
export const syncThemeFromSettings = async (): Promise<void> => {
  try {
    const result = await commands.getAppSettings();
    if (result.status === "ok") {
      applyTheme(
        result.data.theme ?? "system",
        result.data.accent_color ?? "pink",
      );
    }
  } catch (e) {
    console.warn("Failed to sync theme from settings:", e);
  }
};
