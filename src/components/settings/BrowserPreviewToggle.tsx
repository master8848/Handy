import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";
import { commands } from "@/bindings";

/**
 * Experimental toggle for "Show in Browser". Maps to `server_mode_enabled`.
 * Unlike the legacy Server mode toggle which required restart, this one
 * starts/stops the axum server immediately via the browser-server commands.
 * Full port/token settings remain in ServerSettings (also gated behind
 * experimental).
 */
export const BrowserPreviewToggle: React.FC<{
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}> = React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
  const { t } = useTranslation();
  const { getSetting, refreshSettings, isUpdating } = useSettings();
  const enabled = (getSetting("server_mode_enabled") as boolean) ?? false;
  const port = (getSetting("server_port") as number) ?? 17373;
  const [busy, setBusy] = useState(false);

  const handleChange = async (next: boolean) => {
    setBusy(true);
    try {
      if (next) {
        const res = await commands.startBrowserServer();
        if (res.status === "ok") {
          toast.success(t("settings.advanced.toolbar.serverStarted", { url: res.data.url ?? `http://127.0.0.1:${res.data.port}` }));
        } else {
          toast.error(t("settings.advanced.toolbar.serverError", { error: res.error }));
        }
      } else {
        const res = await commands.stopBrowserServer();
        if (res.status === "ok") {
          toast.success(t("settings.advanced.toolbar.serverStoppedToast"));
        } else {
          toast.error(t("settings.advanced.toolbar.serverError", { error: res.error }));
        }
      }
      await refreshSettings();
    } catch (e) {
      toast.error(t("settings.advanced.toolbar.serverError", { error: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  return (
    <ToggleSwitch
      checked={enabled}
      onChange={handleChange}
      isUpdating={busy || isUpdating("server_mode_enabled")}
      label={t("settings.advanced.browserPreview.label")}
      description={t("settings.advanced.browserPreview.description", { port })}
      descriptionMode={descriptionMode}
      grouped={grouped}
    />
  );
});
BrowserPreviewToggle.displayName = "BrowserPreviewToggle";
