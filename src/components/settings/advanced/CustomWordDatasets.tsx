import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import { commands, type CustomWordDataset } from "../../../bindings";
import { useSettings } from "../../../hooks/useSettings";
import { Input } from "../../ui/Input";
import { Button } from "../../ui/Button";
import { SettingContainer } from "../../ui/SettingContainer";
import {
  ChevronDown,
  ChevronRight,
  Download,
  FilePlus,
  Pencil,
  Trash2,
} from "lucide-react";

interface CustomWordDatasetsProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

const sanitizeWord = (word: string) => word.trim().replace(/[<>"']/g, "");

export const CustomWordDatasets: React.FC<CustomWordDatasetsProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating, refreshSettings } =
      useSettings();
    const datasets = getSetting("custom_word_datasets") || [];
    const updating = isUpdating("custom_word_datasets");

    const [newDatasetName, setNewDatasetName] = useState("");
    const [expandedId, setExpandedId] = useState<string | null>(null);
    const [wordInputs, setWordInputs] = useState<Record<string, string>>({});
    const [renameId, setRenameId] = useState<string | null>(null);
    const [renameValue, setRenameValue] = useState("");

    const activeWordCount = datasets.reduce(
      (total, dataset) => total + (dataset.enabled ? dataset.words.length : 0),
      0,
    );

    const saveDatasets = (next: CustomWordDataset[]) => {
      updateSetting("custom_word_datasets", next);
    };

    const handleToggleDataset = (datasetId: string) => {
      saveDatasets(
        datasets.map((dataset) =>
          dataset.id === datasetId
            ? { ...dataset, enabled: !dataset.enabled }
            : dataset,
        ),
      );
    };

    const handleCreateDataset = () => {
      const name = newDatasetName.trim();
      if (!name) {
        return;
      }
      saveDatasets([
        ...datasets,
        {
          id: `user-${Date.now()}`,
          name,
          words: [],
          builtin: false,
          enabled: true,
        },
      ]);
      setNewDatasetName("");
    };

    const handleAddWord = (datasetId: string) => {
      const word = sanitizeWord(wordInputs[datasetId] || "");
      const dataset = datasets.find((d) => d.id === datasetId);
      if (!dataset || !word || word.length > 50) {
        return;
      }
      const exists = dataset.words.some(
        (existing) => existing.toLowerCase() === word.toLowerCase(),
      );
      if (exists) {
        toast.error(
          t("settings.advanced.customWordDatasets.duplicateWord", { word }),
        );
        return;
      }
      saveDatasets(
        datasets.map((d) =>
          d.id === datasetId ? { ...d, words: [...d.words, word] } : d,
        ),
      );
      setWordInputs((inputs) => ({ ...inputs, [datasetId]: "" }));
    };

    const handleRemoveWord = (datasetId: string, word: string) => {
      saveDatasets(
        datasets.map((d) =>
          d.id === datasetId
            ? { ...d, words: d.words.filter((w) => w !== word) }
            : d,
        ),
      );
    };

    const handleStartRename = (dataset: CustomWordDataset) => {
      setRenameId(dataset.id);
      setRenameValue(dataset.name);
    };

    const handleSaveRename = () => {
      if (!renameId) {
        return;
      }
      const name = renameValue.trim();
      if (!name) {
        setRenameId(null);
        return;
      }
      saveDatasets(
        datasets.map((d) => (d.id === renameId ? { ...d, name } : d)),
      );
      setRenameId(null);
    };

    const handleDeleteDataset = async (dataset: CustomWordDataset) => {
      const confirmed = await ask(
        t("settings.advanced.customWordDatasets.deleteConfirm", {
          name: dataset.name,
        }),
        {
          title: t("settings.advanced.customWordDatasets.deleteDataset"),
          kind: "warning",
        },
      );
      if (confirmed) {
        saveDatasets(datasets.filter((d) => d.id !== dataset.id));
      }
    };

    const handleImport = async () => {
      const path = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Text", extensions: ["txt"] }],
      });
      if (typeof path !== "string") {
        return;
      }
      const result = await commands.importCustomWordDataset(path, null);
      if (result.status === "ok") {
        await refreshSettings();
        toast.success(
          t("settings.advanced.customWordDatasets.importSuccess", {
            name: result.data.name,
            count: result.data.words.length,
          }),
        );
      } else {
        toast.error(
          t("settings.advanced.customWordDatasets.importFailed", {
            error: result.error,
          }),
        );
      }
    };

    const handleExport = async (dataset: CustomWordDataset) => {
      const path = await save({
        defaultPath: `${dataset.name}.txt`,
        filters: [{ name: "Text", extensions: ["txt"] }],
      });
      if (!path) {
        return;
      }
      const result = await commands.exportCustomWordDataset(dataset.id, path);
      if (result.status === "ok") {
        toast.success(
          t("settings.advanced.customWordDatasets.exportSuccess", {
            name: dataset.name,
          }),
        );
      } else {
        toast.error(
          t("settings.advanced.customWordDatasets.exportFailed", {
            error: result.error,
          }),
        );
      }
    };

    return (
      <>
        <SettingContainer
          title={t("settings.advanced.customWordDatasets.title")}
          description={t("settings.advanced.customWordDatasets.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <div className="flex items-center gap-2">
            <Input
              type="text"
              className="max-w-44"
              value={newDatasetName}
              onChange={(e) => setNewDatasetName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  handleCreateDataset();
                }
              }}
              placeholder={t(
                "settings.advanced.customWordDatasets.newDatasetPlaceholder",
              )}
              variant="compact"
              disabled={updating}
            />
            <Button
              onClick={handleCreateDataset}
              disabled={!newDatasetName.trim() || updating}
              variant="primary"
              size="md"
            >
              {t("settings.advanced.customWordDatasets.create")}
            </Button>
          </div>
        </SettingContainer>

        <div
          className={`px-4 p-2 ${grouped ? "" : "rounded-lg border border-mid-gray/20"} space-y-2`}
        >
          <div className="flex items-center justify-between">
            <span className="text-sm text-mid-gray">
              {t("settings.advanced.customWordDatasets.activeWords", {
                count: activeWordCount,
              })}
            </span>
            <Button
              onClick={handleImport}
              disabled={updating}
              variant="secondary"
              size="sm"
              className="inline-flex items-center gap-1"
            >
              <FilePlus className="w-3.5 h-3.5" />
              {t("settings.advanced.customWordDatasets.importButton")}
            </Button>
          </div>

          {datasets.length === 0 && (
            <p className="text-sm text-mid-gray py-2">
              {t("settings.advanced.customWordDatasets.emptyList")}
            </p>
          )}

          {datasets.map((dataset) => {
            const isExpanded = expandedId === dataset.id;
            const isRenaming = renameId === dataset.id;
            return (
              <div
                key={dataset.id}
                className="rounded-lg border border-mid-gray/20 overflow-hidden"
              >
                <div className="flex items-center justify-between px-3 py-2 gap-2">
                  <div className="flex items-center gap-2 min-w-0">
                    <button
                      className="text-mid-gray hover:text-logo-primary transition-colors cursor-pointer shrink-0"
                      onClick={() =>
                        setExpandedId(isExpanded ? null : dataset.id)
                      }
                      aria-label={t(
                        isExpanded
                          ? "settings.advanced.customWordDatasets.collapsed"
                          : "settings.advanced.customWordDatasets.expanded",
                      )}
                    >
                      {isExpanded ? (
                        <ChevronDown className="w-4 h-4" />
                      ) : (
                        <ChevronRight className="w-4 h-4" />
                      )}
                    </button>
                    {isRenaming ? (
                      <Input
                        type="text"
                        className="max-w-40"
                        value={renameValue}
                        onChange={(e) => setRenameValue(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") {
                            e.preventDefault();
                            handleSaveRename();
                          }
                          if (e.key === "Escape") {
                            setRenameId(null);
                          }
                        }}
                        variant="compact"
                        autoFocus
                        disabled={updating}
                      />
                    ) : (
                      <span className="text-sm font-medium truncate">
                        {dataset.name}
                      </span>
                    )}
                    {dataset.builtin && (
                      <span className="text-xs px-1.5 py-0.5 rounded bg-mid-gray/15 text-mid-gray">
                        {t("settings.advanced.customWordDatasets.builtinBadge")}
                      </span>
                    )}
                    <span className="text-xs text-mid-gray shrink-0">
                      {t("settings.advanced.customWordDatasets.words", {
                        count: dataset.words.length,
                      })}
                    </span>
                  </div>

                  <div className="flex items-center gap-1 shrink-0">
                    {!dataset.builtin && (
                      <>
                        <button
                          className="text-mid-gray hover:text-logo-primary transition-colors cursor-pointer p-1"
                          onClick={() =>
                            isRenaming
                              ? handleSaveRename()
                              : handleStartRename(dataset)
                          }
                          disabled={updating}
                          aria-label={t(
                            "settings.advanced.customWordDatasets.rename",
                          )}
                        >
                          <Pencil className="w-3.5 h-3.5" />
                        </button>
                        <button
                          className="text-mid-gray hover:text-red-500 transition-colors cursor-pointer p-1"
                          onClick={() => handleDeleteDataset(dataset)}
                          disabled={updating}
                          aria-label={t(
                            "settings.advanced.customWordDatasets.deleteDataset",
                          )}
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </>
                    )}
                    <button
                      className="text-mid-gray hover:text-logo-primary transition-colors cursor-pointer p-1"
                      onClick={() => handleExport(dataset)}
                      disabled={updating}
                      aria-label={t(
                        "settings.advanced.customWordDatasets.export",
                      )}
                    >
                      <Download className="w-3.5 h-3.5" />
                    </button>
                    <label
                      className="flex items-center cursor-pointer"
                      aria-label={t(
                        "settings.advanced.customWordDatasets.toggle",
                      )}
                    >
                      <input
                        type="checkbox"
                        className="sr-only peer"
                        checked={dataset.enabled}
                        disabled={updating}
                        onChange={() => handleToggleDataset(dataset.id)}
                      />
                      <div className="relative w-9 h-5 bg-mid-gray/20 rounded-full peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-logo-primary peer-disabled:opacity-50"></div>
                    </label>
                  </div>
                </div>

                {isExpanded && (
                  <div className="px-3 pb-3 space-y-2 border-t border-mid-gray/15">
                    {dataset.builtin && (
                      <p className="text-xs text-mid-gray pt-2">
                        {t("settings.advanced.customWordDatasets.readOnlyHint")}
                      </p>
                    )}
                    {!dataset.builtin && (
                      <div className="flex items-center gap-2 pt-2">
                        <Input
                          type="text"
                          className="max-w-44"
                          value={wordInputs[dataset.id] || ""}
                          onChange={(e) =>
                            setWordInputs((inputs) => ({
                              ...inputs,
                              [dataset.id]: e.target.value,
                            }))
                          }
                          onKeyDown={(e) => {
                            if (e.key === "Enter") {
                              e.preventDefault();
                              handleAddWord(dataset.id);
                            }
                          }}
                          placeholder={t(
                            "settings.advanced.customWordDatasets.addWordPlaceholder",
                          )}
                          variant="compact"
                          disabled={updating}
                        />
                        <Button
                          onClick={() => handleAddWord(dataset.id)}
                          disabled={
                            !(wordInputs[dataset.id] || "").trim() || updating
                          }
                          variant="primary"
                          size="sm"
                        >
                          {t("settings.advanced.customWordDatasets.add")}
                        </Button>
                      </div>
                    )}
                    <div className="flex flex-wrap gap-1">
                      {dataset.words.map((word) => (
                        <Button
                          key={word}
                          onClick={() =>
                            !dataset.builtin &&
                            handleRemoveWord(dataset.id, word)
                          }
                          disabled={updating || dataset.builtin}
                          variant="secondary"
                          size="sm"
                          className="inline-flex items-center gap-1 cursor-pointer"
                          aria-label={t(
                            "settings.advanced.customWordDatasets.removeWord",
                            { word },
                          )}
                        >
                          <span>{word}</span>
                          {!dataset.builtin && (
                            <svg
                              className="w-3 h-3"
                              fill="none"
                              stroke="currentColor"
                              viewBox="0 0 24 24"
                            >
                              <path
                                strokeLinecap="round"
                                strokeLinejoin="round"
                                strokeWidth={2}
                                d="M6 18L18 6M6 6l12 12"
                              />
                            </svg>
                          )}
                        </Button>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </>
    );
  },
);
