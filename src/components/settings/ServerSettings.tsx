import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { SettingContainer } from "../ui/SettingContainer";
import { Input } from "../ui/Input";
import { Button } from "../ui/Button";
import { useSettings } from "../../hooks/useSettings";
import { commands } from "@/bindings";

export const ServerSettings: React.FC<{ grouped?: boolean }> = ({ grouped = false }) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating, refreshSettings } = useSettings();
  const enabled = (getSetting("server_mode_enabled") as boolean) ?? false;
  const port = (getSetting("server_port") as number) ?? 17373;
  const token = (getSetting("server_auth_token") as string | null) ?? null;
  const experimentalEnabled = (getSetting("experimental_enabled") as boolean) ?? false;

  const [copied, setCopied] = useState(false);
  const [regenLoading, setRegenLoading] = useState(false);

  const handleCopy = async () => {
    if (!token) return;
    try {
      await navigator.clipboard.writeText(token);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // fallback
    }
  };

  const handleRegenerate = async () => {
    setRegenLoading(true);
    try {
      const res = await commands.regenerateServerTokenSetting();
      if (res.status === "ok") {
        await refreshSettings();
      }
    } finally {
      setRegenLoading(false);
    }
  };

  return (
    <div className="space-y-3">
      <ToggleSwitch
        checked={enabled}
        onChange={(v) => updateSetting("server_mode_enabled", v as never)}
        isUpdating={isUpdating("server_mode_enabled")}
        label={t("settings.advanced.serverMode.toggleLabel")}
        description={t("settings.advanced.serverMode.toggleDescription")}
        descriptionMode="tooltip"
        grouped={grouped}
        tooltipPosition="bottom"
      />
      <SettingContainer
        title={t("settings.advanced.serverMode.title")}
        description={t("settings.advanced.serverMode.description", { port })}
        descriptionMode="inline"
        grouped={grouped}
      >
        <div className="w-full text-xs text-mid-gray">
          {t("settings.advanced.serverMode.requiresRestart")}
        </div>
      </SettingContainer>
      <SettingContainer
        title={t("settings.advanced.serverMode.portLabel")}
        description={t("settings.advanced.serverMode.portDescription")}
        descriptionMode="tooltip"
        grouped={grouped}
      >
        <Input
          type="number"
          min={1024}
          max={65535}
          value={String(port)}
          onChange={(e) => {
            const v = parseInt(e.target.value, 10);
            if (!Number.isNaN(v) && v >= 1024 && v <= 65535) {
              updateSetting("server_port", v as never);
            }
          }}
          disabled={isUpdating("server_port")}
          className="w-24"
        />
      </SettingContainer>
      <SettingContainer
        title={t("settings.advanced.serverMode.tokenLabel")}
        description={t("settings.advanced.serverMode.tokenDescription")}
        descriptionMode="tooltip"
        grouped={grouped}
      >
        <div className="flex items-center gap-2 w-full">
          <Input
            value={token ?? ""}
            readOnly
            placeholder="—"
            className="flex-1 font-mono text-xs"
          />
          <Button variant="secondary" size="sm" onClick={handleCopy} disabled={!token}>
            {copied ? t("settings.advanced.serverMode.copied") : t("settings.advanced.serverMode.copyToken")}
          </Button>
          <Button variant="secondary" size="sm" onClick={handleRegenerate} disabled={regenLoading}>
            {t("settings.advanced.serverMode.regenerate")}
          </Button>
        </div>
      </SettingContainer>
      {enabled && (
        <div className="px-4 py-2 text-xs text-amber-600 dark:text-amber-400 bg-amber-50 dark:bg-amber-950/30 rounded-md border border-amber-200 dark:border-amber-800">
          {t("settings.advanced.serverMode.restartHint")}
        </div>
      )}
      {experimentalEnabled && (
        <div className="px-4 py-2 text-xs text-mid-gray bg-mid-gray/5 rounded-md border border-mid-gray/20">
          {t("settings.advanced.serverMode.toolbarNote")}
        </div>
      )}
    </div>
  );
};
