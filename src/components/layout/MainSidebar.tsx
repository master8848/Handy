import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Cog,
  FileAudio,
  History as HistoryIcon,
  Library,
  Mic,
  NotebookPen,
} from "lucide-react";
import HandyTextLogo from "../icons/HandyTextLogo";
import { commands } from "@/bindings";

export type MainNavId =
  | "dictate"
  | "prompt"
  | "library"
  | "history"
  | "transcribe"
  | "settings";

interface NavItem {
  id: MainNavId;
  labelKey: string;
  icon: React.ComponentType<{ className?: string; width?: number; height?: number }>;
}

const NAV: NavItem[] = [
  { id: "dictate", labelKey: "tabs.dictate", icon: Mic },
  { id: "prompt", labelKey: "tabs.prompt", icon: NotebookPen },
  { id: "library", labelKey: "sidebar.promptLibrary", icon: Library },
  { id: "history", labelKey: "sidebar.promptHistory", icon: HistoryIcon },
  { id: "transcribe", labelKey: "sidebar.transcribe", icon: FileAudio },
];

interface MainSidebarProps {
  active: MainNavId;
  onChange: (id: MainNavId) => void;
  collapsed: boolean;
  onToggleCollapsed: () => void;
}

export const MainSidebar: React.FC<MainSidebarProps> = ({
  active,
  onChange,
  collapsed,
  onToggleCollapsed,
}) => {
  const { t } = useTranslation();
  const [counts, setCounts] = useState<{ prompts: number; history: number }>({
    prompts: 0,
    history: 0,
  });

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const [pl, ph] = await Promise.all([
          commands.listPrompts(null),
          commands.listPromptHistory(100),
        ]);
        if (cancelled) return;
        setCounts({
          prompts: pl.status === "ok" ? pl.data.length : 0,
          history: ph.status === "ok" ? ph.data.length : 0,
        });
      } catch {
        // ignore
      }
    };
    void load();
    const id = window.setInterval(load, 8000);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [active]);

  return (
    <div
      className={`hidden sm:flex shrink-0 flex-col border-e border-mid-gray/15 bg-background/50 backdrop-blur supports-[backdrop-filter]:bg-background/60 ${
        collapsed ? "w-[52px]" : "w-[184px]"
      } transition-[width] duration-200`}
    >
      <div className="h-[44px] flex items-center gap-2 px-2 border-b border-mid-gray/10 shrink-0">
        {!collapsed && <HandyTextLogo width={96} className="ml-1" />}
        <button
          type="button"
          onClick={onToggleCollapsed}
          aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
          className="ml-auto w-7 h-7 grid place-items-center rounded-md hover:bg-mid-gray/10 text-text/40 hover:text-text transition-colors"
        >
          <span className="text-[10px] font-bold tracking-widest">{collapsed ? "»" : "«"}</span>
        </button>
      </div>

      <nav className="flex-1 py-2 px-1.5 space-y-0.5 overflow-y-auto">
        {NAV.map((item) => {
          const Icon = item.icon;
          const isActive = active === item.id;
          const badge =
            item.id === "library"
              ? counts.prompts
              : item.id === "history"
                ? counts.history
                : null;
          return (
            <button
              key={item.id}
              type="button"
              onClick={() => onChange(item.id)}
              title={t(item.labelKey)}
              className={`w-full flex items-center gap-2.5 px-2 py-2 rounded-lg text-sm transition-colors ${
                isActive
                  ? "bg-logo-primary/15 text-logo-primary"
                  : "text-text/70 hover:text-text hover:bg-mid-gray/10"
              } ${collapsed ? "justify-center px-1" : ""}`}
            >
              <Icon className="w-4 h-4 shrink-0" />
              {!collapsed && (
                <>
                  <span className="truncate font-medium text-[13px] flex-1 text-left">
                    {t(item.labelKey)}
                  </span>
                  {badge !== null && badge > 0 && (
                    <span className="text-[10px] leading-none px-1.5 py-1 rounded-full bg-mid-gray/15 text-text/60">
                      {badge > 99 ? "99+" : badge}
                    </span>
                  )}
                </>
              )}
            </button>
          );
        })}
      </nav>

      <div className="p-1.5 border-t border-mid-gray/10">
        <button
          type="button"
          onClick={() => onChange("settings")}
          className={`w-full flex items-center gap-2.5 px-2 py-2 rounded-lg text-sm text-text/50 hover:text-text hover:bg-mid-gray/10 transition-colors ${collapsed ? "justify-center px-1" : ""}`}
          title={t("sidebar.advanced")}
        >
          <Cog className="w-4 h-4 shrink-0" />
          {!collapsed && <span className="text-[13px] font-medium">{t("sidebar.advanced")}</span>}
        </button>
        {!collapsed && (
          <p className="text-[10px] text-text/30 px-2 pt-1.5 leading-tight">
            {t("promptStudio.sidebarHint")}
          </p>
        )}
      </div>
    </div>
  );
};
