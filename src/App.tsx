import { useEffect, useState, useRef, type ReactNode } from "react";
import { toast, Toaster } from "sonner";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { platform } from "@tauri-apps/plugin-os";
import {
  checkAccessibilityPermission,
  checkMicrophonePermission,
} from "tauri-plugin-macos-permissions-api";
import { ModelStateEvent, RecordingErrorEvent } from "./lib/types/events";
import "./App.css";
import AccessibilityPermissions from "./components/AccessibilityPermissions";
import SecureInputWarning from "./components/SecureInputWarning";
import Footer from "./components/footer";
import Onboarding, { AccessibilityOnboarding } from "./components/onboarding";
import {
  Sidebar,
  SidebarSection,
  SECTIONS_CONFIG,
  WINDOW_SECTIONS,
  type WindowView,
} from "./components/Sidebar";
import { TopTabBar, type MainTab } from "./components/TopTabBar";
import { PromptWorkbench } from "./components/prompt-workbench/PromptWorkbench";
import { TranscribeFiles } from "./components/transcribe/TranscribeFiles";
import { WhatsNewGate } from "./components/whats-new";
import { PromptPalette } from "./components/prompt-library/PromptPalette";
import { PromptLibraryView } from "./components/prompt-library/PromptLibraryView";
import { HistoryTimeline } from "./components/history/HistoryTimeline";
import { MainSidebar, type MainNavId } from "./components/layout/MainSidebar";
import { Home } from "./components/settings";
import { ScreenshotsPage } from "./components/screenshots/ScreenshotsPage";
import { useSettings } from "./hooks/useSettings";
import { useSettingsStore } from "./stores/settingsStore";
import { commands } from "@/bindings";
import { getLanguageDirection, initializeRTL } from "@/lib/utils/rtl";

type OnboardingStep = "accessibility" | "model" | "done";

/**
 * Auxiliary window views are selected via the `?view=` query parameter baked
 * into the window URL by the `open_app_window` command. No param = main
 * window, which uses the top tab bar instead of a sidebar.
 */
const getWindowView = (): WindowView | null => {
  const view = new URLSearchParams(window.location.search).get("view");
  return view === "settings" || view === "studio" ? view : null;
};

const isScreenshotsView = (): boolean => {
  const params = new URLSearchParams(window.location.search);
  return params.get("view") === "screenshots" || params.get("screenshots") === "1";
};

const renderSettingsContent = (
  section: SidebarSection,
  onNavigate: (section: SidebarSection) => void,
) => {
  const ActiveComponent =
    SECTIONS_CONFIG[section]?.component || SECTIONS_CONFIG.general.component;
  return <ActiveComponent onNavigate={onNavigate} />;
};

function App() {
  const { t, i18n } = useTranslation();
  // Fixed for the lifetime of the window — the view comes from the URL.
  const [windowView] = useState<WindowView | null>(getWindowView);
  const isMainWindow = windowView === null;
  const [onboardingStep, setOnboardingStep] = useState<OnboardingStep | null>(
    isMainWindow ? null : "done",
  );
  // Track if this is a returning user who just needs to grant permissions
  // (vs a new user who needs full onboarding including model selection)
  const [isReturningUser, setIsReturningUser] = useState(false);
  const [currentSection, setCurrentSection] = useState<SidebarSection>(
    windowView === "studio" ? "prompt-library" : "general",
  );
  const getInitialTab = (): MainTab => {
    const params = new URLSearchParams(window.location.search);
    const tabParam = params.get("tab") as MainTab | null;
    if (tabParam && ["dictate", "prompt", "transcribe"].includes(tabParam)) return tabParam;
    try {
      const stored = localStorage.getItem("handy.activeTab") as MainTab | null;
      if (stored && ["dictate", "prompt", "transcribe"].includes(stored)) return stored;
    } catch {
      // ignore
    }
    return "dictate";
  };
  const [activeTab, setActiveTab] = useState<MainTab>(() => getInitialTab());
  const getInitialMainNav = (): MainNavId => {
    const params = new URLSearchParams(window.location.search);
    const nav = params.get("nav") as MainNavId | null;
    if (nav && ["dictate", "prompt", "library", "history", "transcribe", "settings"].includes(nav)) return nav;
    try {
      const stored = localStorage.getItem("handy.mainNav") as MainNavId | null;
      if (stored && ["dictate", "prompt", "library", "history", "transcribe", "settings"].includes(stored)) return stored;
    } catch {}
    // migrate from activeTab
    if (activeTab === "dictate") return "dictate";
    if (activeTab === "prompt") return "prompt";
    if (activeTab === "transcribe") return "transcribe";
    return "dictate";
  };
  const [mainNav, setMainNav] = useState<MainNavId>(() => getInitialMainNav());
  const [sidebarCollapsed, setSidebarCollapsed] = useState(() => {
    try { return localStorage.getItem("handy.sidebarCollapsed") === "1"; } catch { return false; }
  });
  const { settings, updateSetting } = useSettings();

  useEffect(() => {
    try {
      localStorage.setItem("handy.activeTab", activeTab);
    } catch {
      // ignore
    }
    try {
      const url = new URL(window.location.href);
      url.searchParams.set("tab", activeTab);
      window.history.replaceState(null, "", url.toString());
    } catch {
      // ignore
    }
  }, [activeTab]);
  useEffect(() => {
    try { localStorage.setItem("handy.mainNav", mainNav); } catch {}
    try {
      const url = new URL(window.location.href);
      url.searchParams.set("nav", mainNav);
      window.history.replaceState(null, "", url.toString());
    } catch {}
    // keep activeTab in sync so TopTabBar highlight stays sane on legacy ?tab links
    if (mainNav === "dictate") setActiveTab("dictate");
    else if (mainNav === "prompt") setActiveTab("prompt");
    else if (mainNav === "transcribe") setActiveTab("transcribe");
  }, [mainNav]);
  useEffect(() => {
    try { localStorage.setItem("handy.sidebarCollapsed", sidebarCollapsed ? "1" : "0"); } catch {}
  }, [sidebarCollapsed]);
  const direction = getLanguageDirection(i18n.language);
  const refreshAudioDevices = useSettingsStore(
    (state) => state.refreshAudioDevices,
  );
  const refreshOutputDevices = useSettingsStore(
    (state) => state.refreshOutputDevices,
  );
  const hasCompletedPostOnboardingInit = useRef(false);

  // Onboarding status only matters in the main window; settings/studio
  // windows opened before onboarding completes render their content directly.
  useEffect(() => {
    if (isMainWindow) {
      checkOnboardingStatus();
    }
  }, [isMainWindow]);

  // Initialize RTL direction when language changes
  useEffect(() => {
    initializeRTL(i18n.language);
  }, [i18n.language]);

  // Initialize Enigo, shortcuts, and refresh audio devices when the main
  // window app shell loads (main window only — secondary windows don't own
  // global initialization).
  useEffect(() => {
    if (
      isMainWindow &&
      onboardingStep === "done" &&
      !hasCompletedPostOnboardingInit.current
    ) {
      hasCompletedPostOnboardingInit.current = true;
      Promise.all([
        commands.initializeEnigo(),
        commands.initializeShortcuts(),
      ]).catch((e) => {
        console.warn("Failed to initialize:", e);
      });
      refreshAudioDevices();
      refreshOutputDevices();
    }
  }, [
    isMainWindow,
    onboardingStep,
    refreshAudioDevices,
    refreshOutputDevices,
  ]);

  // Handle keyboard shortcuts for debug mode toggle (main window only)
  useEffect(() => {
    if (!isMainWindow) return;
    const handleKeyDown = (event: KeyboardEvent) => {
      // Check for Ctrl+Shift+D (Windows/Linux) or Cmd+Shift+D (macOS)
      const isDebugShortcut =
        event.shiftKey &&
        event.key.toLowerCase() === "d" &&
        (event.ctrlKey || event.metaKey);

      if (isDebugShortcut) {
        event.preventDefault();
        const currentDebugMode = settings?.debug_mode ?? false;
        updateSetting("debug_mode", !currentDebugMode);
      }
    };

    // Add event listener when component mounts
    document.addEventListener("keydown", handleKeyDown);

    // Cleanup event listener when component unmounts
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [isMainWindow, settings?.debug_mode, updateSetting]);

  // Listen for recording errors from the backend and show a toast
  useEffect(() => {
    const unlisten = listen<RecordingErrorEvent>("recording-error", (event) => {
      const { error_type, detail } = event.payload;

      if (error_type === "microphone_permission_denied") {
        const currentPlatform = platform();
        const platformKey = `errors.micPermissionDenied.${currentPlatform}`;
        const description = t(platformKey, {
          defaultValue: t("errors.micPermissionDenied.generic"),
        });
        toast.error(t("errors.micPermissionDeniedTitle"), { description });
      } else if (error_type === "no_input_device") {
        toast.error(t("errors.noInputDeviceTitle"), {
          description: t("errors.noInputDevice"),
        });
      } else {
        toast.error(
          t("errors.recordingFailed", { error: detail ?? "Unknown error" }),
        );
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  // Listen for paste failures and show a toast.
  // The technical error detail is logged to handy.log on the Rust side
  // (see actions.rs `error!("Failed to paste transcription: ...")`),
  // so we show a localized, user-friendly message here instead of the raw error.
  useEffect(() => {
    const unlisten = listen("paste-error", () => {
      toast.error(t("errors.pasteFailedTitle"), {
        description: t("errors.pasteFailed"),
      });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  // Listen for transcription failures and show a toast.
  // The payload is the backend error message (also logged to handy.log).
  useEffect(() => {
    const unlisten = listen<string>("transcription-error", (event) => {
      toast.error(t("errors.transcriptionFailedTitle"), {
        description: event.payload,
      });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  // Listen for model loading failures and show a toast
  useEffect(() => {
    const unlisten = listen<ModelStateEvent>("model-state-changed", (event) => {
      if (event.payload.event_type === "loading_failed") {
        toast.error(
          t("errors.modelLoadFailed", {
            model:
              event.payload.model_name || t("errors.modelLoadFailedUnknown"),
          }),
          {
            description: event.payload.error,
          },
        );
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  const revealMainWindowForPermissions = async () => {
    try {
      await commands.showMainWindowCommand();
    } catch (e) {
      console.warn("Failed to show main window for permission onboarding:", e);
    }
  };

  // Prompt library/history now live inside the Prompt Studio persona (activeTab="prompt").
  // In the main window, navigating to those sections switches the persona tab instead
  // of opening a separate OS window. The studio window is kept for backward
  // compatibility but all new flows should use the in-window persona.
  const handleMainWindowNavigate = (section: SidebarSection) => {
    if (section === "prompt-library") { setMainNav("library"); return; }
    if (section === "prompt-history") { setMainNav("history"); return; }
    if (section === "transcribe") { setMainNav("transcribe"); return; }
    if (section === "home") { setMainNav("dictate"); return; }
    commands.openAppWindow("settings").catch((e) => {
      console.warn("Failed to open settings window:", e);
    });
  };

  const handleMainNavChange = (nav: MainNavId) => {
    if (nav === "settings") {
      commands.openAppWindow("settings").catch((e) => console.warn("Failed to open settings window:", e));
      return;
    }
    setMainNav(nav);
  };

  const openSettingsWindow = () => handleMainWindowNavigate("general");

  const checkOnboardingStatus = async () => {
    try {
      const settingsResult = await commands.getAppSettings();
      const hasCompletedOnboarding =
        settingsResult.status === "ok" &&
        settingsResult.data.onboarding_completed === true;
      const currentPlatform = platform();

      if (hasCompletedOnboarding) {
        // Returning user - check if they need to grant permissions first
        setIsReturningUser(true);

        if (currentPlatform === "macos") {
          try {
            let [hasAccessibility, hasMicrophone] = await Promise.all([
              checkAccessibilityPermission(),
              checkMicrophonePermission(),
            ]);
            // Fallback: `AXIsProcessTrusted()` can return false after a reinstall
            // with the same bundle ID until restart / re-trust, even though the
            // app is already ticked in System Settings. Enigo init is ground truth
            // — if it succeeds, we are trusted regardless of the plugin check.
            if (!hasAccessibility) {
              try {
                const res = await commands.initializeEnigo();
                if (res.status === "ok") hasAccessibility = true;
              } catch {
                // keep original value
              }
            }
            if (!hasAccessibility || !hasMicrophone) {
              await revealMainWindowForPermissions();
              setOnboardingStep("accessibility");
              return;
            }
          } catch (e) {
            console.warn("Failed to check macOS permissions:", e);
            // If we can't check, proceed to main app and let them fix it there
          }
        }

        if (currentPlatform === "windows") {
          try {
            const microphoneStatus =
              await commands.getWindowsMicrophonePermissionStatus();
            if (
              microphoneStatus.supported &&
              microphoneStatus.overall_access === "denied"
            ) {
              await revealMainWindowForPermissions();
              setOnboardingStep("accessibility");
              return;
            }
          } catch (e) {
            console.warn("Failed to check Windows microphone permissions:", e);
            // If we can't check, proceed to main app and let them fix it there
          }
        }

        setOnboardingStep("done");
      } else {
        // New user - start full onboarding
        setIsReturningUser(false);
        setOnboardingStep("accessibility");
      }
    } catch (error) {
      console.error("Failed to check onboarding status:", error);
      setOnboardingStep("accessibility");
    }
  };

  const handleAccessibilityComplete = () => {
    // Returning users already have models, skip to main app
    // New users need to select a model
    setOnboardingStep(isReturningUser ? "done" : "model");
  };

  const handleModelSelected = () => {
    // Transition to main app - user has started a download
    setOnboardingStep("done");
  };

  const [screenshotsView] = useState<boolean>(() => isScreenshotsView());

  // Rendered once around every step below (including onboarding) so
  // toast.error() calls surface to the user. sonner renders via a portal, so
  // its position in the tree doesn't affect layout. Without this, errors during
  // onboarding (e.g. a model download failing because blob.handy.computer is
  // unreachable) are silently swallowed and the wizard just appears to "blink".
  // The theme follows the applied theme setting (App re-renders on settings
  // change); `system` lets sonner follow the OS like the rest of the UI.
  const toaster = (
    <Toaster
      theme={settings?.theme ?? "system"}
      toastOptions={{
        unstyled: true,
        classNames: {
          toast:
            "bg-background border border-mid-gray/20 rounded-lg shadow-lg px-4 py-3 flex items-center gap-3 text-sm",
          title: "font-medium",
          description: "text-mid-gray",
          actionButton:
            "px-2 py-1 text-xs font-medium rounded-lg border bg-mid-gray/10 border-mid-gray/20 hover:bg-background-ui/30 hover:border-logo-primary cursor-pointer whitespace-nowrap",
        },
      }}
    />
  );

  // Screenshots gallery bypasses onboarding (dev-only, no Tauri required).
  // Reachable via `?view=screenshots` or `?screenshots=1`. Documented URL:
  // http://localhost:5173/?view=screenshots
  if (screenshotsView) {
    return (
      <>
        {toaster}
        <ScreenshotsPage />
      </>
    );
  }

  // Still checking onboarding status (main window only)
  if (onboardingStep === null) {
    return null;
  }

  // Select the content for the current step. The Toaster is rendered once, in a
  // stable wrapper around this node, so crossing between onboarding steps and
  // the main app never remounts it (which would drop any in-flight toast).
  let content: ReactNode;
  if (onboardingStep === "accessibility") {
    content = (
      <AccessibilityOnboarding onComplete={handleAccessibilityComplete} />
    );
  } else if (onboardingStep === "model") {
    content = <Onboarding onModelSelected={handleModelSelected} />;
  } else if (isMainWindow) {
    content = (
      <div dir={direction} className="h-screen flex flex-col select-none cursor-default">
        <WhatsNewGate />
        <TopTabBar activeTab={activeTab} onTabChange={setActiveTab} onOpenSettings={openSettingsWindow} />
        <div className="flex-1 flex min-h-0 overflow-hidden">
          <MainSidebar
            active={mainNav}
            onChange={handleMainNavChange}
            collapsed={sidebarCollapsed}
            onToggleCollapsed={() => setSidebarCollapsed((v) => !v)}
          />
          <div className="flex-1 overflow-y-auto">
            <div className="flex flex-col items-center p-4 gap-4">
              <AccessibilityPermissions />
              <SecureInputWarning />
              {mainNav === "dictate" && <Home showTitle={false} onNavigate={handleMainWindowNavigate} />}
              {mainNav === "prompt" && <PromptWorkbench />}
              {mainNav === "library" && (
                <PromptLibraryView />
              )}
              {mainNav === "history" && (
                <HistoryTimeline onNavigate={handleMainWindowNavigate} />
              )}
              {mainNav === "transcribe" && <TranscribeFiles />}
            </div>
          </div>
        </div>
        <Footer />
      </div>
    );
  } else {
    // Settings / prompt studio auxiliary windows: sidebar layout restricted to
    // their own sections.
    const sidebarSections = windowView
      ? WINDOW_SECTIONS[windowView]
      : undefined;
    const fallbackSection = sidebarSections?.[0] ?? "general";
    const activeSection = sidebarSections?.includes(currentSection)
      ? currentSection
      : fallbackSection;
    content = (
      <div
        dir={direction}
        className="h-screen flex flex-col select-none cursor-default"
      >
        {/* Main content area that takes remaining space */}
        <div className="flex-1 flex overflow-hidden">
          <Sidebar
            activeSection={activeSection}
            onSectionChange={setCurrentSection}
            sections={sidebarSections}
          />
          {/* Scrollable content area */}
          <div className="flex-1 flex flex-col overflow-hidden">
            <div className="flex-1 overflow-y-auto">
              <div className="flex flex-col items-center p-4 gap-4">
                {renderSettingsContent(currentSection, setCurrentSection)}
              </div>
            </div>
          </div>
        </div>
        {/* Fixed footer at bottom */}
        <Footer />
      </div>
    );
  }

  return (
    <>
      {toaster}
      {content}
      <PromptPalette />
    </>
  );
}

export default App;
