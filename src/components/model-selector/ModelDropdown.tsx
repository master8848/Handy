import React from "react";
import { useTranslation } from "react-i18next";
import { X } from "lucide-react";
import type { ModelInfo } from "@/bindings";
import {
  getTranslatedModelName,
  getTranslatedModelDescription,
} from "../../lib/utils/modelTranslation";

interface ModelDropdownProps {
  models: ModelInfo[];
  currentModelId: string;
  loadedModels: string[];
  onModelSelect: (modelId: string) => void;
  onUnload: (modelId: string) => void;
}

const ModelDropdown: React.FC<ModelDropdownProps> = ({
  models,
  currentModelId,
  loadedModels,
  onModelSelect,
  onUnload,
}) => {
  const { t } = useTranslation();
  const downloadedModels = models.filter((m) => m.is_downloaded);

  const handleModelClick = (modelId: string) => {
    onModelSelect(modelId);
  };

  const handleUnloadClick = (e: React.MouseEvent, modelId: string) => {
    e.stopPropagation();
    onUnload(modelId);
  };

  return (
    <div className="absolute bottom-full start-0 mb-2 w-64 max-h-[60vh] overflow-y-auto bg-background border border-mid-gray/20 rounded-lg shadow-lg py-2 z-50">
      {downloadedModels.length > 0 ? (
        <div>
          {downloadedModels.map((model) => (
            <div
              key={model.id}
              onClick={() => handleModelClick(model.id)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  handleModelClick(model.id);
                }
              }}
              tabIndex={0}
              role="button"
              className={`w-full px-3 py-2 text-start hover:bg-mid-gray/10 transition-colors cursor-pointer focus:outline-none ${
                currentModelId === model.id
                  ? "bg-logo-primary/10 text-logo-primary"
                  : ""
              }`}
            >
              <div className="flex items-center justify-between">
                <div>
                  <div className="text-sm text-text/80">
                    {getTranslatedModelName(model, t)}
                    {model.is_custom && (
                      <span className="ms-1.5 text-[10px] font-medium text-text/40 uppercase">
                        {t("modelSelector.custom")}
                      </span>
                    )}
                    {(model.engine_type === "OsSpeech" ||
                      model.id === "os-speech") && (
                      <span className="ms-1.5 text-[10px] font-medium text-text/40 uppercase">
                        {t("modelSelector.builtIn")}
                      </span>
                    )}
                    {model.supports_streaming && (
                      <span className="ms-1.5 text-[10px] font-medium text-logo-primary/70 uppercase">
                        {t("modelSelector.streaming")}
                      </span>
                    )}
                  </div>
                  <div className="text-xs text-text/40 italic pe-4">
                    {getTranslatedModelDescription(model, t)}
                  </div>
                </div>
                {currentModelId === model.id && (
                  <div className="text-xs text-logo-primary">
                    {t("modelSelector.active")}
                  </div>
                )}
                {currentModelId !== model.id &&
                  loadedModels.includes(model.id) && (
                    <div className="flex items-center gap-2 shrink-0">
                      <span className="text-xs text-text/50">
                        {t("modelSelector.loaded")}
                      </span>
                      <button
                        type="button"
                        onClick={(e) => handleUnloadClick(e, model.id)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter" || e.key === " ") {
                            e.preventDefault();
                            e.stopPropagation();
                            onUnload(model.id);
                          }
                        }}
                        title={t("modelSelector.unloadTooltip")}
                        aria-label={t("modelSelector.unloadTooltip")}
                        tabIndex={0}
                        className="text-text/40 hover:text-text/80 transition-colors focus:outline-none"
                      >
                        <X className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  )}
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div className="px-3 py-2 text-sm text-text/60">
          {t("modelSelector.noModelsAvailable")}
        </div>
      )}
    </div>
  );
};

export default ModelDropdown;
