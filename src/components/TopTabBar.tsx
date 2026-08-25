import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  FileAudio,
  Mic,
  NotebookPen,
  SlidersHorizontal,
  Globe,
  Play,
  Square,
  ExternalLink,
  Settings,
} from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import HandyTextLogo from "./icons/HandyTextLogo";
import { useSettings } from "@/hooks/useSettings";
import { commands } from "@/bindings";
import { useSettingsStore } from "@/stores/settingsStore";

export type MainTab = "dictate" | "prompt" | "transcribe";

interface TopTabBarProps {
  activeTab: MainTab;
  onTabChange: (tab: MainTab) => void;
  onOpenSettings: () => void;
  /** Optional toolbar slot on the right (server controls, model status, etc.). */
  toolbarSlot?: React.ReactNode;
}

const TAB_ICONS: Record<MainTab, React.ComponentType<{ className?: string }>> = {
  dictate: Mic,
  prompt: NotebookPen,
  transcribe: FileAudio,
};

const TAB_LABEL_KEYS: Record<MainTab, string> = {
  dictate: "tabs.dictate",
  prompt: "tabs.prompt",
  transcribe: "tabs.transcribe",
};

export const TopTabBar: React.FC<TopTabBarProps> = ({
  activeTab,
  onTabChange,
  onOpenSettings,
  toolbarSlot,
}) => {
  const { t } = useTranslation();
  const { getSetting } = useSettings();
  const refreshSettings = useSettingsStore((s) => s.refreshSettings);
  const experimentalEnabled = (getSetting("experimental_enabled") as boolean) ?? false;

  const [menuOpen, setMenuOpen] = useState(false);
  const [serverRunning, setServerRunning] = useState(false);
  const [serverUrl, setServerUrl] = useState<string | null>(null);
  const [serverBusy, setServerBusy] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  const fetchStatus = async () => {
    try {
      const status = await commands.getBrowserServerStatus();
      setServerRunning(status.running);
      setServerUrl(status.url ?? null);
    } catch {
      // ignore
    }
  };

  useEffect(() => {
    if (experimentalEnabled) void fetchStatus();
  }, [experimentalEnabled]);

  useEffect(() => {
    if (!menuOpen || !experimentalEnabled) return;
    void fetchStatus();
  }, [menuOpen, experimentalEnabled]);

  useEffect(() => {
    if (!menuOpen) return;
    const onDown = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(false);
      }
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [menuOpen]);

  const handleToggleServer = async () => {
    setServerBusy(true);
    try {
      if (serverRunning) {
        const res = await commands.stopBrowserServer();
        if (res.status === "ok") {
          setServerRunning(false);
          setServerUrl(res.data.url ?? serverUrl);
          toast.success(t("settings.advanced.toolbar.serverStoppedToast"));
          await refreshSettings();
        } else {
          toast.error(t("settings.advanced.toolbar.serverError", { error: res.error }));
        }
      } else {
        const res = await commands.startBrowserServer();
        if (res.status === "ok") {
          setServerRunning(true);
          setServerUrl(res.data.url ?? `http://127.0.0.1:${res.data.port}`);
          toast.success(t("settings.advanced.toolbar.serverStarted", { url: res.data.url ?? "" }));
          await refreshSettings();
        } else {
          toast.error(t("settings.advanced.toolbar.serverError", { error: res.error }));
        }
      }
    } catch (e) {
      toast.error(t("settings.advanced.toolbar.serverError", { error: String(e) }));
    } finally {
      setServerBusy(false);
    }
  };

  const handleOpenInBrowser = async () => {
    if (!serverUrl) return;
    try {
      await openUrl(serverUrl);
    } catch (e) {
      toast.error(t("settings.advanced.toolbar.serverError", { error: String(e) }));
    }
  };

  return (
    <div className="flex items-center gap-3 px-3 h-[44px] shrink-0 border-b border-mid-gray/20 bg-background">
      <HandyTextLogo width={88} className="shrink-0" />
      <div className="hidden sm:block w-px h-5 bg-mid-gray/20 mx-1 shrink-0" aria-hidden />
      <div
        role="tablist"
        className="flex items-center bg-mid-gray/10 rounded-full p-1 gap-1 border border-mid-gray/10"
        aria-orientation="horizontal"
        aria-label={t("tabs.dictate")}
      >
        {(Object.keys(TAB_LABEL_KEYS) as MainTab[]).map((tab) => {
          const Icon = TAB_ICONS[tab];
          const isActive = activeTab === tab;
          return (
            <button
              key={tab}
              type="button"
              role="tab"
              aria-selected={isActive}
              onClick={() => onTabChange(tab)}
              className={`flex items-center gap-1.5 px-3.5 py-1.5 rounded-full text-xs font-medium transition-colors cursor-pointer ${
                isActive
                  ? "bg-logo-primary/15 text-logo-primary"
                  : "text-text/60 hover:text-text hover:bg-mid-gray/10"
              }`}
            >
              <Icon className="w-3.5 h-3.5 shrink-0" />
              {t(TAB_LABEL_KEYS[tab])}
            </button>
          );
        })}
      </div>
      <div className="flex-1" />
      {toolbarSlot ? <div className="hidden sm:flex items-center gap-1.5">{toolbarSlot}</div> : null}
      <div className="relative shrink-0" ref={menuRef}>
        <button
          type="button"
          onClick={() => setMenuOpen((v) => !v)}
          title={t("home.openSettings")}
          aria-label={t("home.openSettings")}
          aria-haspopup="menu"
          aria-expanded={menuOpen}
          className="w-7 h-7 grid place-items-center rounded-lg hover:bg-mid-gray/10 text-text/60 hover:text-text transition-colors cursor-pointer"
        >
          <SlidersHorizontal className="w-4 h-4" />
        </button>
        {menuOpen && (
          <div
            role="menu"
            className="absolute right-0 top-full mt-2 w-72 bg-background border border-mid-gray/20 rounded-lg shadow-lg z-50 py-1 overflow-hidden"
          >
            <button
              type="button"
              role="menuitem"
              onClick={() => {
                setMenuOpen(false);
                onOpenSettings();
              }}
              className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-mid-gray/10 text-left cursor-pointer"
            >
              <Settings className="w-4 h-4 shrink-0 text-text/60" />
              {t("settings.advanced.toolbar.preferences")}
            </button>
            {experimentalEnabled && (
              <>
                <div className="h-px bg-mid-gray/15 my-1" aria-hidden />
                <div className="px-3 py-2">
                  <div className="flex items-center gap-2 text-xs font-medium">
                    <span
                      className={`w-2 h-2 rounded-full shrink-0 ${serverRunning ? "bg-green-500" : "bg-red-500"}`}
                      aria-hidden
                    />
                    <Globe className="w-3.5 h-3.5 shrink-0 text-text/60" />
                    <span className="truncate">{t("settings.advanced.toolbar.showInBrowser")}</span>
                    <span className={`ml-auto text-[11px] ${serverRunning ? "text-green-600" : "text-mid-gray"}`}>
                      {serverRunning
                        ? t("settings.advanced.toolbar.serverRunning")
                        : t("settings.advanced.toolbar.serverStopped")}
                    </span>
                  </div>
                  {serverUrl && (
                    <div className="text-[11px] font-mono text-mid-gray truncate mt-1">{serverUrl}</div>
                  )}
                  <div className="flex gap-2 mt-2">
                    <button
                      type="button"
                      onClick={handleToggleServer}
                      disabled={serverBusy}
                      className="flex-1 inline-flex items-center justify-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium border border-mid-gray/20 hover:bg-mid-gray/10 disabled:opacity-50 cursor-pointer"
                    >
                      {serverRunning ? <Square className="w-3.5 h-3.5" /> : <Play className="w-3.5 h-3.5" />}
                      {serverRunning
                        ? t("settings.advanced.toolbar.stopServer")
                        : t("settings.advanced.toolbar.startServer")}
                    </button>
                    <button
                      type="button"
                      onClick={handleOpenInBrowser}
                      disabled={!serverRunning || !serverUrl}
                      className="inline-flex items-center justify-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium border border-mid-gray/20 hover:bg-mid-gray/10 disabled:opacity-50 cursor-pointer"
                    >
                      <ExternalLink className="w-3.5 h-3.5" />
                      {t("settings.advanced.toolbar.openInBrowser")}
                    </button>
                  </div>
                </div>
              </>
            )}
          </div>
        )}
      </div>
    </div>
  );
};
