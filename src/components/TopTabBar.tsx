import React from "react";
import { useTranslation } from "react-i18next";
import { FileAudio, Library, Mic, NotebookPen, Settings } from "lucide-react";
import HandyTextLogo from "./icons/HandyTextLogo";

export type MainTab = "dictate" | "prompt" | "transcribe";

interface TopTabBarProps {
  activeTab: MainTab;
  onTabChange: (tab: MainTab) => void;
  onOpenStudio: () => void;
  onOpenSettings: () => void;
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

/**
 * Affinity-style top tab bar for the main window: Handy logo on the left,
 * the Dictate / Prompt / Transcribe tabs next to it, and quick-access icon
 * buttons (prompt library studio + settings) on the right.
 */
export const TopTabBar: React.FC<TopTabBarProps> = ({
  activeTab,
  onTabChange,
  onOpenStudio,
  onOpenSettings,
}) => {
  const { t } = useTranslation();

  return (
    <div className="flex items-center gap-2 px-3 pt-2 pb-0 border-b border-mid-gray/20">
      <HandyTextLogo width={96} className="me-3 mb-1.5 shrink-0" />
      <div
        role="tablist"
        className="flex items-stretch gap-1 self-end"
        aria-orientation="horizontal"
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
              className={`flex items-center gap-1.5 px-3.5 py-2 text-sm font-medium rounded-t-lg transition-colors cursor-pointer border border-b-0 ${
                isActive
                  ? "bg-logo-primary/15 border-mid-gray/20 text-logo-primary"
                  : "border-transparent text-text/60 hover:bg-mid-gray/10 hover:text-text opacity-85 hover:opacity-100"
              }`}
            >
              <Icon className="w-4 h-4 shrink-0" />
              {t(TAB_LABEL_KEYS[tab])}
            </button>
          );
        })}
      </div>
      <div className="flex-1" />
      <button
        type="button"
        onClick={onOpenStudio}
        title={t("home.openPromptLibrary")}
        aria-label={t("home.openPromptLibrary")}
        className="p-2 mb-1.5 rounded-lg text-text/60 hover:bg-mid-gray/10 hover:text-text transition-colors cursor-pointer"
      >
        <Library className="w-4.5 h-4.5" />
      </button>
      <button
        type="button"
        onClick={onOpenSettings}
        title={t("home.openSettings")}
        aria-label={t("home.openSettings")}
        className="p-2 mb-1.5 rounded-lg text-text/60 hover:bg-mid-gray/10 hover:text-text transition-colors cursor-pointer"
      >
        <Settings className="w-4.5 h-4.5" />
      </button>
    </div>
  );
};
