import { useEffect, useState, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { platform } from "@tauri-apps/plugin-os";
import { relaunch } from "@tauri-apps/plugin-process";
import {
  checkAccessibilityPermission,
  requestAccessibilityPermission,
  checkMicrophonePermission,
  requestMicrophonePermission,
} from "tauri-plugin-macos-permissions-api";
import { toast } from "sonner";
import { commands } from "@/bindings";
import { useSettingsStore } from "@/stores/settingsStore";
import HandyTextLogo from "../icons/HandyTextLogo";
import { Keyboard, Mic, Check, Loader2 } from "lucide-react";

interface AccessibilityOnboardingProps {
  onComplete: () => void;
}

type PermissionStatus = "checking" | "needed" | "waiting" | "granted";
type PermissionPlatform = "macos" | "windows" | "other";

interface PermissionsState {
  accessibility: PermissionStatus;
  microphone: PermissionStatus;
}

const RESTART_HINT_THRESHOLD = 8; // polls (~8s) before showing restart guidance on macOS

const AccessibilityOnboarding: React.FC<AccessibilityOnboardingProps> = ({
  onComplete,
}) => {
  const { t } = useTranslation();
  const refreshAudioDevices = useSettingsStore(
    (state) => state.refreshAudioDevices,
  );
  const refreshOutputDevices = useSettingsStore(
    (state) => state.refreshOutputDevices,
  );
  const [permissionPlatform, setPermissionPlatform] =
    useState<PermissionPlatform | null>(null);
  const [permissions, setPermissions] = useState<PermissionsState>({
    accessibility: "checking",
    microphone: "checking",
  });
  const [pollCount, setPollCount] = useState(0);
  const [isManualChecking, setIsManualChecking] = useState(false);
  const pollingRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const errorCountRef = useRef<number>(0);
  const pollCountRef = useRef<number>(0);
  const MAX_POLLING_ERRORS = 3;

  const isMacOS = permissionPlatform === "macos";
  const isWindows = permissionPlatform === "windows";
  const showMicrophonePermission = isMacOS || isWindows;
  const showAccessibilityPermission = isMacOS;

  // Keep refs in sync so intervals / focus handlers never read stale closures
  const permissionPlatformRef = useRef(permissionPlatform);
  useEffect(() => {
    permissionPlatformRef.current = permissionPlatform;
  }, [permissionPlatform]);

  const permissionsRef = useRef(permissions);
  useEffect(() => {
    permissionsRef.current = permissions;
  }, [permissions]);

  const allGranted = isMacOS
    ? permissions.accessibility === "granted" &&
      permissions.microphone === "granted"
    : isWindows
      ? permissions.microphone === "granted"
      : true;

  const showRestartHint =
    isMacOS &&
    permissions.accessibility === "waiting" &&
    pollCount >= RESTART_HINT_THRESHOLD;

  const completeOnboarding = useCallback(async () => {
    await Promise.all([refreshAudioDevices(), refreshOutputDevices()]);
    timeoutRef.current = setTimeout(() => onComplete(), 300);
  }, [onComplete, refreshAudioDevices, refreshOutputDevices]);

  const completeOnboardingRef = useRef(completeOnboarding);
  useEffect(() => {
    completeOnboardingRef.current = completeOnboarding;
  }, [completeOnboarding]);

  const hasWindowsMicrophoneAccess = useCallback(async (): Promise<boolean> => {
    const microphoneStatus =
      await commands.getWindowsMicrophonePermissionStatus();

    if (!microphoneStatus.supported) {
      return true;
    }

    return microphoneStatus.overall_access !== "denied";
  }, []);

  const hasWindowsMicrophoneAccessRef = useRef(hasWindowsMicrophoneAccess);
  useEffect(() => {
    hasWindowsMicrophoneAccessRef.current = hasWindowsMicrophoneAccess;
  }, [hasWindowsMicrophoneAccess]);

  const stopPolling = useCallback(() => {
    if (pollingRef.current) {
      clearInterval(pollingRef.current);
      pollingRef.current = null;
    }
  }, []);

  const checkPermissionsNow = useCallback(async () => {
    const currentPlatform = permissionPlatformRef.current;
    if (currentPlatform === null || currentPlatform === "other") return false;

    try {
      if (currentPlatform === "windows") {
        const microphoneGranted =
          await hasWindowsMicrophoneAccessRef.current();

        if (microphoneGranted) {
          setPermissions((prev) => ({ ...prev, microphone: "granted" }));
          stopPolling();
          await completeOnboardingRef.current();
          return true;
        }
        // Still not granted — keep waiting UI, but allow manual retry
        setPermissions((prev) =>
          prev.microphone === "waiting"
            ? prev
            : { ...prev, microphone: "waiting" },
        );
        errorCountRef.current = 0;
        return false;
      }

      // macOS: check both — with Enigo fallback for stale AXIsProcessTrusted after reinstall
      let [accessibilityGranted, microphoneGranted] = await Promise.all([
        checkAccessibilityPermission(),
        checkMicrophonePermission(),
      ]);
      if (!accessibilityGranted) {
        try {
          const res = await commands.initializeEnigo();
          if (res.status === "ok") accessibilityGranted = true;
        } catch {
          // keep plugin result
        }
      }

      let anyGranted = false;
      setPermissions((prev) => {
        const newState = { ...prev };
        if (accessibilityGranted && prev.accessibility !== "granted") {
          newState.accessibility = "granted";
          anyGranted = true;
          Promise.all([
            commands.initializeEnigo(),
            commands.initializeShortcuts(),
          ]).catch((e) => {
            console.warn("Failed to initialize after permission grant:", e);
          });
        } else if (accessibilityGranted) {
          anyGranted = true;
        }
        if (microphoneGranted && prev.microphone !== "granted") {
          newState.microphone = "granted";
          anyGranted = true;
        }
        return newState;
      });

      // Track polls while still waiting for at least one permission —
      // drives the restart-required hint. `AXIsProcessTrusted()` often
      // returns false until the app restarts, so polling forever will
      // never succeed otherwise.
      const stillWaiting =
        !accessibilityGranted || !microphoneGranted;
      if (stillWaiting) {
        pollCountRef.current += 1;
        setPollCount(pollCountRef.current);
      } else {
        // Reset when something was granted (prevents stale hint)
        void anyGranted;
      }

      if (accessibilityGranted && microphoneGranted) {
        stopPolling();
        await completeOnboardingRef.current();
        return true;
      }

      // Single granted but not both: keep polling for the other
      errorCountRef.current = 0;
      return false;
    } catch (error) {
      console.error("Error checking permissions:", error);
      errorCountRef.current += 1;
      if (errorCountRef.current >= MAX_POLLING_ERRORS) {
        stopPolling();
        toast.error(t("onboarding.permissions.errors.checkFailed"));
      }
      return false;
    }
  }, [stopPolling, t]);

  // Polling for permissions after user clicks a button — driven by checkPermissionsNow
  // so focus/manual re-check and interval share the same logic (no stale closures).
  const startPolling = useCallback(() => {
    if (pollingRef.current) return;
    if (permissionPlatformRef.current === null) return;

    // Reset poll counter when (re)starting
    pollCountRef.current = 0;
    setPollCount(0);
    errorCountRef.current = 0;

    pollingRef.current = setInterval(() => {
      void checkPermissionsNow();
    }, 1000);
  }, [checkPermissionsNow]);

  // Re-check when the app regains focus (user returns from System Settings).
  // This is the primary recovery path when the 1s interval is throttled in the
  // background or when AXIsProcessTrusted only flips on next run-loop.
  // Also handles reinstall-with-same-bundle-ID where TCC already ticked but
  // initial check returned false — focus after DMG copy should re-check.
  useEffect(() => {
    if (!isMacOS && !isWindows) return;

    const handleFocusCheck = () => {
      const p = permissionsRef.current;
      if (
        p.accessibility === "waiting" ||
        p.accessibility === "needed" ||
        p.microphone === "waiting" ||
        p.microphone === "needed"
      ) {
        void checkPermissionsNow();
      }
    };

    const handleVisibility = () => {
      if (document.visibilityState === "visible") handleFocusCheck();
    };

    window.addEventListener("focus", handleFocusCheck);
    document.addEventListener("visibilitychange", handleVisibility);
    return () => {
      window.removeEventListener("focus", handleFocusCheck);
      document.removeEventListener("visibilitychange", handleVisibility);
    };
  }, [isMacOS, isWindows, checkPermissionsNow]);

  // Check platform and permission status on mount
  useEffect(() => {
    const currentPlatform = platform();
    const nextPlatform: PermissionPlatform =
      currentPlatform === "macos"
        ? "macos"
        : currentPlatform === "windows"
          ? "windows"
          : "other";

    setPermissionPlatform(nextPlatform);

    // Skip immediately on unsupported platforms
    if (nextPlatform === "other") {
      onComplete();
      return;
    }

    const checkInitial = async () => {
      if (nextPlatform === "macos") {
        try {
          let [accessibilityGranted, microphoneGranted] = await Promise.all([
            checkAccessibilityPermission(),
            checkMicrophonePermission(),
          ]);

          // Fallback for reinstall-with-same-bundle-ID: AXIsProcessTrusted can
          // still return false after DMG copy until restart, even though TCC
          // shows ticked. Enigo init is ground truth — if it succeeds, treat as granted.
          if (!accessibilityGranted) {
            try {
              const res = await commands.initializeEnigo();
              if (res.status === "ok") accessibilityGranted = true;
            } catch {
              // keep original value
            }
          }

          // If accessibility is granted, initialize Enigo and shortcuts
          if (accessibilityGranted) {
            try {
              await Promise.all([
                commands.initializeEnigo(),
                commands.initializeShortcuts(),
              ]);
            } catch (e) {
              console.warn("Failed to initialize after permission grant:", e);
            }
          }

          const newState: PermissionsState = {
            accessibility: accessibilityGranted ? "granted" : "needed",
            microphone: microphoneGranted ? "granted" : "needed",
          };

          setPermissions(newState);

          if (accessibilityGranted && microphoneGranted) {
            await completeOnboarding();
          }
        } catch (error) {
          console.error("Failed to check macOS permissions:", error);
          toast.error(t("onboarding.permissions.errors.checkFailed"));
          setPermissions({
            accessibility: "needed",
            microphone: "needed",
          });
        }

        return;
      }

      try {
        const microphoneGranted = await hasWindowsMicrophoneAccess();

        setPermissions({
          accessibility: "granted",
          microphone: microphoneGranted ? "granted" : "needed",
        });

        if (microphoneGranted) {
          await completeOnboarding();
        }
      } catch (error) {
        console.warn("Failed to check Windows microphone permissions:", error);
        setPermissions({
          accessibility: "granted",
          microphone: "granted",
        });
        await completeOnboarding();
      }
    };

    checkInitial();
  }, [completeOnboarding, hasWindowsMicrophoneAccess, onComplete, t]);

  // Cleanup polling and timeouts on unmount
  useEffect(() => {
    return () => {
      if (pollingRef.current) {
        clearInterval(pollingRef.current);
      }
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, []);

  const handleGrantAccessibility = async () => {
    try {
      await requestAccessibilityPermission();
      setPermissions((prev) => ({ ...prev, accessibility: "waiting" }));
      startPolling();
    } catch (error) {
      console.error("Failed to request accessibility permission:", error);
      toast.error(t("onboarding.permissions.errors.requestFailed"));
    }
  };

  const handleGrantMicrophone = async () => {
    try {
      if (isWindows) {
        await commands.openMicrophonePrivacySettings();
      } else {
        await requestMicrophonePermission();
      }

      setPermissions((prev) => ({ ...prev, microphone: "waiting" }));
      startPolling();
    } catch (error) {
      console.error("Failed to request microphone permission:", error);
      toast.error(t("onboarding.permissions.errors.requestFailed"));
    }
  };

  const handleCheckAgain = async () => {
    setIsManualChecking(true);
    try {
      const done = await checkPermissionsNow();
      if (!done) {
        // Start/ensure polling is active for continued waiting
        startPolling();
      }
    } finally {
      setIsManualChecking(false);
    }
  };

  const handleRestart = async () => {
    try {
      await relaunch();
    } catch (e) {
      console.error("Failed to relaunch:", e);
      toast.error(t("onboarding.permissions.errors.checkFailed"));
    }
  };

  const handleContinueAnyway = async () => {
    stopPolling();
    await completeOnboarding();
  };

  const isChecking =
    permissionPlatform === null ||
    (isMacOS &&
      permissions.accessibility === "checking" &&
      permissions.microphone === "checking") ||
    (isWindows && permissions.microphone === "checking");

  // Still checking platform/initial permissions
  if (isChecking) {
    return (
      <div className="h-screen w-screen flex items-center justify-center">
        <Loader2 className="w-8 h-8 animate-spin text-text/50" />
      </div>
    );
  }

  // All permissions granted - show success briefly
  if (allGranted) {
    return (
      <div className="h-screen w-screen flex flex-col items-center justify-center gap-4">
        <div className="p-4 rounded-full bg-emerald-500/20">
          <Check className="w-12 h-12 text-emerald-400" />
        </div>
        <p className="text-lg font-medium text-text">
          {t("onboarding.permissions.allGranted")}
        </p>
      </div>
    );
  }

  // Show permissions request screen
  return (
    <div className="h-screen w-screen flex flex-col p-6 gap-6 items-center justify-center">
      <div className="flex flex-col items-center gap-2">
        <HandyTextLogo width={200} />
      </div>

      <div className="max-w-md w-full flex flex-col items-center gap-4">
        <div className="text-center mb-2">
          <h2 className="text-xl font-semibold text-text mb-2">
            {t("onboarding.permissions.title")}
          </h2>
          <p className="text-text/70">
            {t("onboarding.permissions.description")}
          </p>
        </div>

        {/* Microphone Permission Card */}
        {showMicrophonePermission && (
          <div className="w-full p-4 rounded-lg bg-white/5 border border-mid-gray/20">
            <div className="flex items-center gap-4">
              <div className="p-3 rounded-full bg-logo-primary/20 shrink-0">
                <Mic className="w-6 h-6 text-logo-primary" />
              </div>
              <div className="flex-1 min-w-0">
                <h3 className="font-medium text-text">
                  {t("onboarding.permissions.microphone.title")}
                </h3>
                <p className="text-sm text-text/60 mb-3">
                  {t("onboarding.permissions.microphone.description")}
                </p>
                {permissions.microphone === "granted" ? (
                  <div className="flex items-center gap-2 text-emerald-400 text-sm">
                    <Check className="w-4 h-4" />
                    {t("onboarding.permissions.granted")}
                  </div>
                ) : permissions.microphone === "waiting" ? (
                  <div className="flex flex-col gap-2">
                    <div className="flex items-center gap-2 text-text/50 text-sm">
                      <Loader2 className="w-4 h-4 animate-spin" />
                      {t("onboarding.permissions.waiting")}
                    </div>
                    <button
                      onClick={handleCheckAgain}
                      disabled={isManualChecking}
                      className="px-3 py-1.5 rounded-lg bg-white/10 hover:bg-white/15 border border-mid-gray/20 text-text text-sm font-medium transition-colors disabled:opacity-50 w-fit flex items-center gap-2"
                    >
                      {isManualChecking && (
                        <Loader2 className="w-3 h-3 animate-spin" />
                      )}
                      {t("onboarding.permissions.checkAgain")}
                    </button>
                  </div>
                ) : (
                  <button
                    onClick={handleGrantMicrophone}
                    className="px-4 py-2 rounded-lg bg-logo-primary hover:bg-logo-primary/90 text-white text-sm font-medium transition-colors"
                  >
                    {isWindows
                      ? t("accessibility.openSettings")
                      : t("onboarding.permissions.grant")}
                  </button>
                )}
              </div>
            </div>
          </div>
        )}

        {/* Accessibility Permission Card */}
        {showAccessibilityPermission && (
          <div className="w-full p-4 rounded-lg bg-white/5 border border-mid-gray/20">
            <div className="flex items-center gap-4">
              <div className="p-3 rounded-full bg-logo-primary/20 shrink-0">
                <Keyboard className="w-6 h-6 text-logo-primary" />
              </div>
              <div className="flex-1 min-w-0">
                <h3 className="font-medium text-text">
                  {t("onboarding.permissions.accessibility.title")}
                </h3>
                <p className="text-sm text-text/60 mb-3">
                  {t("onboarding.permissions.accessibility.description")}
                </p>
                {permissions.accessibility === "granted" ? (
                  <div className="flex items-center gap-2 text-emerald-400 text-sm">
                    <Check className="w-4 h-4" />
                    {t("onboarding.permissions.granted")}
                  </div>
                ) : permissions.accessibility === "waiting" ? (
                  <div className="flex flex-col gap-2">
                    <div className="flex items-center gap-2 text-text/50 text-sm">
                      <Loader2 className="w-4 h-4 animate-spin" />
                      {t("onboarding.permissions.waiting")}
                    </div>
                    <div className="flex flex-wrap gap-2">
                      <button
                        onClick={handleCheckAgain}
                        disabled={isManualChecking}
                        className="px-3 py-1.5 rounded-lg bg-white/10 hover:bg-white/15 border border-mid-gray/20 text-text text-sm font-medium transition-colors disabled:opacity-50 flex items-center gap-2"
                      >
                        {isManualChecking && (
                          <Loader2 className="w-3 h-3 animate-spin" />
                        )}
                        {t("onboarding.permissions.checkAgain")}
                      </button>
                      {showRestartHint && (
                        <button
                          onClick={handleGrantAccessibility}
                          className="px-3 py-1.5 rounded-lg bg-white/10 hover:bg-white/15 border border-mid-gray/20 text-text text-sm font-medium transition-colors"
                        >
                          {t("accessibility.openSettings")}
                        </button>
                      )}
                    </div>
                    {showRestartHint && (
                      <div className="flex flex-col gap-2 pt-1">
                        <p className="text-xs text-amber-300/80 leading-relaxed">
                          {t("onboarding.permissions.restartHint")}
                        </p>
                        <div className="flex flex-wrap gap-2">
                          <button
                            onClick={handleRestart}
                            className="px-3 py-1.5 rounded-lg bg-logo-primary hover:bg-logo-primary/90 text-white text-sm font-medium transition-colors"
                          >
                            {t("onboarding.permissions.restart")}
                          </button>
                          <button
                            onClick={handleContinueAnyway}
                            className="px-3 py-1.5 rounded-lg bg-white/10 hover:bg-white/15 border border-mid-gray/20 text-text text-sm font-medium transition-colors"
                          >
                            {t("onboarding.permissions.continueAnyway")}
                          </button>
                        </div>
                      </div>
                    )}
                  </div>
                ) : (
                  <button
                    onClick={handleGrantAccessibility}
                    className="px-4 py-2 rounded-lg bg-logo-primary hover:bg-logo-primary/90 text-white text-sm font-medium transition-colors"
                  >
                    {t("onboarding.permissions.grant")}
                  </button>
                )}
              </div>
            </div>
          </div>
        )}

        {/* Global continue when stuck in waiting (e.g. mic waiting on Windows) */}
        {(permissions.accessibility === "waiting" ||
          permissions.microphone === "waiting") &&
          !showRestartHint &&
          pollCount >= RESTART_HINT_THRESHOLD && (
            <button
              onClick={handleContinueAnyway}
              className="text-sm text-text/60 hover:text-text underline underline-offset-4 transition-colors"
            >
              {t("onboarding.permissions.continueAnyway")}
            </button>
          )}
      </div>
    </div>
  );
};

export default AccessibilityOnboarding;
