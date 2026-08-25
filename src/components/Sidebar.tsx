import React from "react";
import { useTranslation } from "react-i18next";
import {
  Cog,
  FlaskConical,
  History,
  Home as HomeIcon,
  Info,
  NotebookPen,
  Palette,
  Sparkles,
  Cpu,
  FileAudio,
  Library,
} from "lucide-react";
import HandyTextLogo from "./icons/HandyTextLogo";
import HandyHand from "./icons/HandyHand";
import { useSettings } from "../hooks/useSettings";
import {
  GeneralSettings,
  AdvancedSettings,
  HistorySettings,
  DebugSettings,
  AboutSettings,
  PostProcessingSettings,
  ModelsSettings,
  AppearanceSettings,
  Home,
  PromptHistory,
} from "./settings";
import { PromptLibraryView } from "./prompt-library/PromptLibraryView";
import type { AppSection } from "../lib/types/navigation";
import { TranscribeFiles } from "./transcribe/TranscribeFiles";

export type SidebarSection = keyof typeof SECTIONS_CONFIG;

/**
 * Auxiliary window views that render a filtered sidebar (via `?view=`).
 * @deprecated Legacy — main window now uses the Affinity persona shell
 * (TopTabBar pills). Settings/Studio windows remain for backward compat
 * only; new flows should switch `activeTab` in the main window instead.
 */
export type WindowView = "settings" | "studio";

/**
 * Sections shown per auxiliary window view, in display order. The main window
 * uses the tab bar instead and passes no filter.
 */
export const WINDOW_SECTIONS: Record<WindowView, readonly SidebarSection[]> = {
  settings: [
    "general",
    "appearance",
    "history",
    "models",
    "advanced",
    "postprocessing",
    "debug",
    "about",
  ],
  studio: ["prompt-library", "prompt-history"],
};

interface IconProps {
  width?: number | string;
  height?: number | string;
  size?: number | string;
  className?: string;
  [key: string]: any;
}

interface SectionConfig {
  labelKey: string;
  icon: React.ComponentType<IconProps>;
  component: React.ComponentType<{
    onNavigate?: (section: AppSection) => void;
  }>;
  enabled: (settings: any) => boolean;
}

export const SECTIONS_CONFIG = {
  home: {
    labelKey: "sidebar.home",
    icon: HomeIcon,
    component: Home,
    enabled: () => true,
  },
  "prompt-library": {
    labelKey: "sidebar.promptLibrary",
    icon: Library,
    component: PromptLibraryView,
    enabled: () => true,
  },
  "prompt-history": {
    labelKey: "sidebar.promptHistory",
    icon: NotebookPen,
    component: PromptHistory,
    enabled: () => true,
  },
  general: {
    labelKey: "sidebar.general",
    icon: HandyHand,
    component: GeneralSettings,
    enabled: () => true,
  },
  appearance: {
    labelKey: "sidebar.appearance",
    icon: Palette,
    component: AppearanceSettings,
    enabled: () => true,
  },
  history: {
    labelKey: "sidebar.history",
    icon: History,
    component: HistorySettings,
    enabled: () => true,
  },
  models: {
    labelKey: "sidebar.models",
    icon: Cpu,
    component: ModelsSettings,
    enabled: () => true,
  },
  transcribe: {
    labelKey: "sidebar.transcribe",
    icon: FileAudio,
    component: TranscribeFiles,
    enabled: () => true,
  },
  advanced: {
    labelKey: "sidebar.advanced",
    icon: Cog,
    component: AdvancedSettings,
    enabled: () => true,
  },
  postprocessing: {
    labelKey: "sidebar.postProcessing",
    icon: Sparkles,
    component: PostProcessingSettings,
    enabled: (settings) => settings?.post_process_enabled ?? false,
  },
  debug: {
    labelKey: "sidebar.debug",
    icon: FlaskConical,
    component: DebugSettings,
    enabled: (settings) => settings?.debug_mode ?? false,
  },
  about: {
    labelKey: "sidebar.about",
    icon: Info,
    component: AboutSettings,
    enabled: () => true,
  },
} as const satisfies Record<string, SectionConfig>;

interface SidebarProps {
  activeSection: SidebarSection;
  onSectionChange: (section: SidebarSection) => void;
  /** Restrict the sidebar to these sections (e.g. settings/studio windows). */
  sections?: readonly SidebarSection[];
}

export const Sidebar: React.FC<SidebarProps> = ({
  activeSection,
  onSectionChange,
  sections,
}) => {
  const { t } = useTranslation();
  const { settings } = useSettings();

  const availableSections = Object.entries(SECTIONS_CONFIG)
    .filter(([id, config]) =>
      sections ? sections.includes(id as SidebarSection) : true,
    )
    .filter(([_, config]) => config.enabled(settings))
    .map(([id, config]) => ({ id: id as SidebarSection, ...config }));

  return (
    <div className="flex flex-col w-40 h-full border-e border-mid-gray/20 items-center px-2">
      <HandyTextLogo width={120} className="m-4" />
      <div className="flex flex-col w-full items-center gap-1 pt-2 border-t border-mid-gray/20">
        {availableSections.map((section) => {
          const Icon = section.icon;
          const isActive = activeSection === section.id;

          return (
            <div
              key={section.id}
              className={`flex gap-2 items-center p-2 w-full rounded-lg cursor-pointer transition-colors ${
                isActive
                  ? "bg-logo-primary/80"
                  : "hover:bg-mid-gray/20 hover:opacity-100 opacity-85"
              }`}
              onClick={() => onSectionChange(section.id)}
            >
              <Icon width={24} height={24} className="shrink-0" />
              <p
                className="text-sm font-medium truncate"
                title={t(section.labelKey)}
              >
                {t(section.labelKey)}
              </p>
            </div>
          );
        })}
      </div>
    </div>
  );
};
