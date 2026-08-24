import React, { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ask } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  ChevronDown,
  ExternalLink,
  Globe,
  Loader2,
  RefreshCw,
  Search,
} from "lucide-react";
import type { ModelCardStatus } from "@/components/onboarding";
import { ModelCard } from "@/components/onboarding";
import { useModelStore } from "@/stores/modelStore";
import { useSettings } from "@/hooks/useSettings";
import { commands } from "@/bindings";
import { ToggleSwitch } from "@/components/ui/ToggleSwitch";
import { Button } from "@/components/ui/Button";
import {
  getLanguageLabel,
  MODEL_CAPABILITY_LANGUAGES,
  supportsLanguageCode,
} from "@/lib/constants/languages.ts";
import type { ModelInfo } from "@/bindings";

// Apple guides for the macOS permissions the OS speech model needs.
const SPEECH_RECOGNITION_HELP_URL =
  "https://support.apple.com/guide/mac-help/control-access-to-speech-recognition-on-mac-mchl48fbbd25/mac";
const MICROPHONE_HELP_URL =
  "https://support.apple.com/guide/mac-help/control-access-to-your-microphone-on-mac-mchlp6d0b7e2/mac";

// check if model supports a language based on its supported_languages list
const modelSupportsLanguage = (model: ModelInfo, langCode: string): boolean => {
  return supportsLanguageCode(model.supported_languages, langCode);
};

// Legacy models are the blob (Url-sourced) .bin/ONNX downloads, superseded by
// the catalog GGUFs. They stay runnable when already on disk, but we no longer
// advertise the download.
const isLegacyModel = (model: ModelInfo): boolean =>
  typeof model.source === "object" && "Url" in model.source;

export const ModelsSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const [switchingModelId, setSwitchingModelId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [languageFilter, setLanguageFilter] = useState("all");
  const [languageDropdownOpen, setLanguageDropdownOpen] = useState(false);
  const [languageSearch, setLanguageSearch] = useState("");
  const [osAuthError, setOsAuthError] = useState(false);
  const [grantingPermission, setGrantingPermission] = useState(false);
  const [permissionRequested, setPermissionRequested] = useState(false);
  const languageDropdownRef = useRef<HTMLDivElement>(null);
  const languageSearchInputRef = useRef<HTMLInputElement>(null);
  const {
    models,
    currentModel,
    loadedModels,
    downloadingModels,
    downloadProgress,
    downloadStats,
    verifyingModels,
    extractingModels,
    loading,
    isRescanning,
    downloadModel,
    cancelDownload,
    selectModel,
    deleteModel,
    unloadModel,
    rescanLocalModels,
  } = useModelStore();

  // click outside handler for language dropdown
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        languageDropdownRef.current &&
        !languageDropdownRef.current.contains(event.target as Node)
      ) {
        setLanguageDropdownOpen(false);
        setLanguageSearch("");
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  // focus search input when dropdown opens
  useEffect(() => {
    if (languageDropdownOpen && languageSearchInputRef.current) {
      languageSearchInputRef.current.focus();
    }
  }, [languageDropdownOpen]);

  // filtered languages for dropdown (exclude "auto")
  const filteredLanguages = useMemo(() => {
    return MODEL_CAPABILITY_LANGUAGES.filter((lang) =>
      lang.label.toLowerCase().includes(languageSearch.toLowerCase()),
    );
  }, [languageSearch]);

  // Get selected language label
  const selectedLanguageLabel = useMemo(() => {
    if (languageFilter === "all") {
      return t("settings.models.filters.allLanguages");
    }
    return getLanguageLabel(languageFilter) || "";
  }, [languageFilter, t]);

  const getModelStatus = (modelId: string): ModelCardStatus => {
    if (modelId in extractingModels) {
      return "extracting";
    }
    if (modelId in verifyingModels) {
      return "verifying";
    }
    if (modelId in downloadingModels) {
      return "downloading";
    }
    if (switchingModelId === modelId) {
      return "switching";
    }
    if (loadedModels.includes(modelId) && modelId !== currentModel) {
      return "loaded";
    }
    if (modelId === currentModel) {
      return "active";
    }
    const model = models.find((m: ModelInfo) => m.id === modelId);
    if (model?.is_downloaded) {
      return "available";
    }
    return "downloadable";
  };

  const getDownloadProgress = (modelId: string): number | undefined => {
    const progress = downloadProgress[modelId];
    return progress?.percentage;
  };

  const getDownloadSpeed = (modelId: string): number | undefined => {
    const stats = downloadStats[modelId];
    return stats?.speed;
  };

  const handleModelSelect = async (modelId: string) => {
    setSwitchingModelId(modelId);
    try {
      const success = await selectModel(modelId);
      // macOS: loading os-speech fails until speech recognition permission is
      // granted. The backend marks this with a stable "[os_speech_auth_required]"
      // prefix (older builds used the English "authorization required" wording).
      if (!success && modelId === "os-speech") {
        const storeError = useModelStore.getState().error ?? "";
        if (
          storeError.includes("[os_speech_auth_required]") ||
          storeError.includes("authorization required")
        ) {
          setOsAuthError(true);
        }
      }
    } finally {
      setSwitchingModelId(null);
    }
  };

  const handleModelUnload = async (modelId: string) => {
    try {
      await unloadModel(modelId);
    } catch (err) {
      console.error(`Failed to unload model ${modelId}:`, err);
    }
  };

  const handleOpenSpeechSettings = async () => {
    try {
      await commands.openSpeechRecognitionSettings();
    } catch (err) {
      console.error("Failed to open speech recognition settings:", err);
    }
  };

  const handleGrantPermission = async () => {
    setGrantingPermission(true);
    try {
      await commands.osSpeechRequestAuthorization();
      setPermissionRequested(true);
      setOsAuthError(false);
      await handleModelSelect("os-speech");
    } catch (err) {
      console.error("Failed to request OS speech authorization:", err);
    } finally {
      setGrantingPermission(false);
    }
  };

  const handleModelDownload = async (modelId: string) => {
    await downloadModel(modelId);
  };

  const handleModelDelete = async (modelId: string) => {
    const model = models.find((m: ModelInfo) => m.id === modelId);
    // The OS speech engine is a virtual model — nothing to delete.
    if (model?.engine_type === "OsSpeech" || modelId === "os-speech") {
      return;
    }
    const modelName = model?.name || modelId;
    const isActive = modelId === currentModel;

    const confirmed = await ask(
      isActive
        ? t("settings.models.deleteActiveConfirm", { modelName })
        : t("settings.models.deleteConfirm", { modelName }),
      {
        title: t("settings.models.deleteTitle"),
        kind: "warning",
      },
    );

    if (confirmed) {
      try {
        await deleteModel(modelId);
      } catch (err) {
        console.error(`Failed to delete model ${modelId}:`, err);
      }
    }
  };

  const handleModelCancel = async (modelId: string) => {
    try {
      await cancelDownload(modelId);
    } catch (err) {
      console.error(`Failed to cancel download for ${modelId}:`, err);
    }
  };

  // Filter models by search query (name + description) and language filter
  const filteredModels = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    return models.filter((model: ModelInfo) => {
      // Hide deprecated legacy (.bin/ONNX) downloads unless already on disk.
      if (isLegacyModel(model) && !model.is_downloaded) return false;
      if (languageFilter !== "all") {
        if (!modelSupportsLanguage(model, languageFilter)) return false;
      }
      if (q) {
        const haystack = `${model.name} ${model.description}`.toLowerCase();
        if (!haystack.includes(q)) return false;
      }
      return true;
    });
  }, [models, languageFilter, searchQuery]);

  // Split filtered models into downloaded (including custom) and available sections
  const { downloadedModels, availableModels } = useMemo(() => {
    const downloaded: ModelInfo[] = [];
    const available: ModelInfo[] = [];

    for (const model of filteredModels) {
      if (
        model.is_custom ||
        model.is_downloaded ||
        model.id in downloadingModels ||
        model.id in extractingModels
      ) {
        downloaded.push(model);
      } else {
        available.push(model);
      }
    }

    // Sort: active model first, then loaded (in-memory) models, then
    // non-custom, then custom at the bottom
    downloaded.sort((a, b) => {
      if (a.id === currentModel) return -1;
      if (b.id === currentModel) return 1;
      const aLoaded = loadedModels.includes(a.id);
      const bLoaded = loadedModels.includes(b.id);
      if (aLoaded !== bLoaded) return aLoaded ? -1 : 1;
      if (a.is_custom !== b.is_custom) return a.is_custom ? 1 : -1;
      return 0;
    });

    return {
      downloadedModels: downloaded,
      availableModels: available,
    };
  }, [
    filteredModels,
    downloadingModels,
    extractingModels,
    currentModel,
    loadedModels,
  ]);

  if (loading) {
    return (
      <div className="max-w-3xl w-full mx-auto">
        <div className="flex items-center justify-center py-16">
          <div className="w-8 h-8 border-2 border-logo-primary border-t-transparent rounded-full animate-spin" />
        </div>
      </div>
    );
  }

  return (
    <div className="max-w-3xl w-full mx-auto space-y-4">
      <div className="mb-4">
        <h1 className="text-xl font-semibold mb-2">
          {t("settings.models.title")}
        </h1>
        <p className="text-sm text-text/60">
          {t("settings.models.description")}
        </p>
      </div>

      {/* Multi-model loading toggle — keep several models in memory at once */}
      <div className="bg-background border border-mid-gray/20 rounded-lg overflow-visible">
        <ToggleSwitch
          checked={getSetting("multi_model_loading") || false}
          onChange={(enabled) => updateSetting("multi_model_loading", enabled)}
          isUpdating={isUpdating("multi_model_loading")}
          label={t("settings.models.multiModel.title")}
          description={t("settings.models.multiModel.description")}
          descriptionMode="inline"
        />
      </div>

      {/* Search bar — filter the catalog by name or description */}
      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-text/40 pointer-events-none" />
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          placeholder={t("settings.models.searchPlaceholder")}
          className="w-full pl-9 pr-3 py-2 text-sm bg-mid-gray/10 border border-mid-gray/40 rounded-lg focus:outline-none focus:ring-1 focus:ring-logo-primary placeholder:text-text/40"
        />
      </div>

      {/* macOS speech recognition permission banner (os-speech model) */}
      {osAuthError && (
        <div className="flex flex-col gap-2 p-3 rounded-lg border border-warning/40 bg-warning/10">
          <p className="text-sm text-text/80">
            {t("settings.models.osSpeech.authRequired")}
          </p>
          <div className="flex flex-wrap items-center gap-2">
            <Button
              variant="secondary"
              size="sm"
              onClick={handleOpenSpeechSettings}
              className="shrink-0"
            >
              {t("settings.models.osSpeech.openSystemSettings")}
            </Button>
            <Button
              variant="secondary"
              size="sm"
              disabled={grantingPermission}
              onClick={handleGrantPermission}
              className="shrink-0"
            >
              {grantingPermission ? (
                <Loader2 className="w-3.5 h-3.5 animate-spin" />
              ) : (
                t(
                  permissionRequested
                    ? "settings.models.osSpeech.retry"
                    : "settings.models.osSpeech.grantPermission",
                )
              )}
            </Button>
            <button
              type="button"
              onClick={() => openUrl(SPEECH_RECOGNITION_HELP_URL)}
              className="flex items-center gap-1 text-xs text-logo-primary hover:underline"
            >
              <ExternalLink className="w-3 h-3" />
              {t("settings.models.osSpeech.learnMoreSpeech")}
            </button>
            <button
              type="button"
              onClick={() => openUrl(MICROPHONE_HELP_URL)}
              className="flex items-center gap-1 text-xs text-logo-primary hover:underline"
            >
              <ExternalLink className="w-3 h-3" />
              {t("settings.models.osSpeech.learnMoreMic")}
            </button>
          </div>
        </div>
      )}

      {filteredModels.length > 0 ? (
        <div className="space-y-6">
          {/* Downloaded Models Section — header always visible so filter stays accessible */}
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <h2 className="text-sm font-medium text-text/60">
                {t("settings.models.yourModels")}
              </h2>
              <div className="flex items-center gap-2">
                {/* Rescan local sources for models added outside Handy */}
                <button
                  type="button"
                  onClick={() => rescanLocalModels()}
                  disabled={isRescanning}
                  title={t("settings.models.rescan.tooltip")}
                  className="flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium rounded-lg bg-mid-gray/10 text-text/60 hover:bg-mid-gray/20 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  <RefreshCw
                    className={`w-3.5 h-3.5 ${isRescanning ? "animate-spin" : ""}`}
                  />
                  <span>{t("settings.models.rescan.label")}</span>
                </button>
                {/* Language filter dropdown */}
                <div className="relative" ref={languageDropdownRef}>
                  <button
                    type="button"
                    onClick={() =>
                      setLanguageDropdownOpen(!languageDropdownOpen)
                    }
                    className={`flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium rounded-lg transition-colors ${
                      languageFilter !== "all"
                        ? "bg-logo-primary/20 text-logo-primary"
                        : "bg-mid-gray/10 text-text/60 hover:bg-mid-gray/20"
                    }`}
                  >
                    <Globe className="w-3.5 h-3.5" />
                    <span className="max-w-[120px] truncate">
                      {selectedLanguageLabel}
                    </span>
                    <ChevronDown
                      className={`w-3.5 h-3.5 transition-transform ${
                        languageDropdownOpen ? "rotate-180" : ""
                      }`}
                    />
                  </button>

                  {languageDropdownOpen && (
                    <div className="absolute top-full right-0 mt-1 w-56 bg-background border border-mid-gray/80 rounded-lg shadow-lg z-50 overflow-hidden">
                      <div className="p-2 border-b border-mid-gray/40">
                        <input
                          ref={languageSearchInputRef}
                          type="text"
                          value={languageSearch}
                          onChange={(e) => setLanguageSearch(e.target.value)}
                          onKeyDown={(e) => {
                            if (
                              e.key === "Enter" &&
                              filteredLanguages.length > 0
                            ) {
                              setLanguageFilter(filteredLanguages[0].value);
                              setLanguageDropdownOpen(false);
                              setLanguageSearch("");
                            } else if (e.key === "Escape") {
                              setLanguageDropdownOpen(false);
                              setLanguageSearch("");
                            }
                          }}
                          placeholder={t(
                            "settings.general.language.searchPlaceholder",
                          )}
                          className="w-full px-2 py-1 text-sm bg-mid-gray/10 border border-mid-gray/40 rounded-md focus:outline-none focus:ring-1 focus:ring-logo-primary"
                        />
                      </div>
                      <div className="max-h-48 overflow-y-auto">
                        <button
                          type="button"
                          onClick={() => {
                            setLanguageFilter("all");
                            setLanguageDropdownOpen(false);
                            setLanguageSearch("");
                          }}
                          className={`w-full px-3 py-1.5 text-sm text-left transition-colors ${
                            languageFilter === "all"
                              ? "bg-logo-primary/20 text-logo-primary font-semibold"
                              : "hover:bg-mid-gray/10"
                          }`}
                        >
                          {t("settings.models.filters.allLanguages")}
                        </button>
                        {filteredLanguages.map((lang) => (
                          <button
                            key={lang.value}
                            type="button"
                            onClick={() => {
                              setLanguageFilter(lang.value);
                              setLanguageDropdownOpen(false);
                              setLanguageSearch("");
                            }}
                            className={`w-full px-3 py-1.5 text-sm text-left transition-colors ${
                              languageFilter === lang.value
                                ? "bg-logo-primary/20 text-logo-primary font-semibold"
                                : "hover:bg-mid-gray/10"
                            }`}
                          >
                            {lang.label}
                          </button>
                        ))}
                        {filteredLanguages.length === 0 && (
                          <div className="px-3 py-2 text-sm text-text/50 text-center">
                            {t("settings.general.language.noResults")}
                          </div>
                        )}
                      </div>
                    </div>
                  )}
                </div>
              </div>
            </div>
            {downloadedModels.map((model: ModelInfo) => (
              <ModelCard
                key={model.id}
                model={model}
                status={getModelStatus(model.id)}
                onSelect={handleModelSelect}
                onDownload={handleModelDownload}
                onDelete={handleModelDelete}
                onUnload={handleModelUnload}
                onCancel={handleModelCancel}
                downloadProgress={getDownloadProgress(model.id)}
                downloadSpeed={getDownloadSpeed(model.id)}
                showRecommended={false}
              />
            ))}
          </div>

          {/* Available Models Section */}
          {availableModels.length > 0 && (
            <div className="space-y-3">
              <h2 className="text-sm font-medium text-text/60">
                {t("settings.models.availableModels")}
              </h2>
              {availableModels.map((model: ModelInfo) => (
                <ModelCard
                  key={model.id}
                  model={model}
                  status={getModelStatus(model.id)}
                  onSelect={handleModelSelect}
                  onDownload={handleModelDownload}
                  onDelete={handleModelDelete}
                  onUnload={handleModelUnload}
                  onCancel={handleModelCancel}
                  downloadProgress={getDownloadProgress(model.id)}
                  downloadSpeed={getDownloadSpeed(model.id)}
                  showRecommended={true}
                />
              ))}
            </div>
          )}
        </div>
      ) : (
        <div className="text-center py-8 text-text/50">
          {t("settings.models.noModelsMatch")}
        </div>
      )}
    </div>
  );
};
