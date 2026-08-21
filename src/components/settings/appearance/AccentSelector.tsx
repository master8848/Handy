import React from "react";
import { useTranslation } from "react-i18next";
import { SettingContainer } from "../../ui/SettingContainer";
import { useSettings } from "@/hooks/useSettings";
import { ACCENT_OPTIONS, applyTheme, getStoredTheme } from "@/lib/utils/theme";
import type { AccentColor } from "@/bindings";

// Swatch colors shown in the picker. Source of truth for the actual rendered
// colors is src/styles/theme.css (`:root[data-accent=...]` blocks) — keep the
// hex values here in sync with the light/dark logo-primary pairs there.
const ACCENT_SWATCHES: Record<AccentColor, { light: string; dark: string }> = {
  pink: { light: "#faa2ca", dark: "#f28cbb" },
  orange: { light: "#ffb37e", dark: "#ff9f57" },
  purple: { light: "#c9a6f5", dark: "#b98cf0" },
  green: { light: "#a8e3b8", dark: "#8fd8a3" },
  blue: { light: "#a5c7f5", dark: "#8eb6f2" },
  red: { light: "#f5a9a9", dark: "#f28d8d" },
};

interface AccentSelectorProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const AccentSelector: React.FC<AccentSelectorProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { settings, updateSetting } = useSettings();

    const themeSetting = settings?.theme ?? "system";
    const isDark =
      themeSetting === "dark" ||
      (themeSetting === "system" &&
        window.matchMedia("(prefers-color-scheme: dark)").matches);
    const currentAccent: AccentColor = settings?.accent_color ?? "pink";

    const handleAccentChange = (accent: AccentColor) => {
      applyTheme(getStoredTheme(), accent);
      updateSetting("accent_color", accent);
    };

    return (
      <SettingContainer
        title={t("settings.appearance.accent.title")}
        description={t("settings.appearance.accent.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      >
        <div className="flex items-center gap-2">
          {ACCENT_OPTIONS.map((accent) => {
            const swatch = ACCENT_SWATCHES[accent];
            const selected = currentAccent === accent;
            const label = t(`settings.appearance.accent.options.${accent}`);
            return (
              <button
                key={accent}
                type="button"
                onClick={() => handleAccentChange(accent)}
                aria-label={label}
                aria-pressed={selected}
                title={label}
                className={`w-6 h-6 rounded-full transition-shadow ${
                  selected
                    ? "ring-2 ring-text ring-offset-2 ring-offset-background"
                    : "hover:ring-2 hover:ring-mid-gray/50 hover:ring-offset-2 hover:ring-offset-background"
                }`}
                style={{ backgroundColor: isDark ? swatch.dark : swatch.light }}
              />
            );
          })}
        </div>
      </SettingContainer>
    );
  },
);

AccentSelector.displayName = "AccentSelector";
