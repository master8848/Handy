import React, { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Trash2 } from "lucide-react";
import { useSettings } from "../../../hooks/useSettings";

interface Props {
  grouped?: boolean;
  descriptionMode?: "tooltip" | "inline";
}

export const TextReplacements: React.FC<Props> = ({
  grouped,
  descriptionMode = "tooltip",
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const [draft, setDraft] = useState<{ find: string; replace: string } | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);

  const replacements = useMemo(
    () => getSetting("text_replacements") ?? [],
    [getSetting],
  );

  const persist = useCallback(
    async (next: typeof replacements) => {
      setError(null);
      try {
        await updateSetting("text_replacements", next);
      } catch (e) {
        setError(String(e));
      }
    },
    [updateSetting],
  );

  const confirmAdd = useCallback(() => {
    if (!draft) return;
    if (!draft.find.trim()) {
      setError(t("settings.advanced.textReplacements.emptyFind"));
      return;
    }
    persist([
      ...replacements,
      {
        id: `repl-${Date.now()}`,
        find: draft.find,
        replace: draft.replace,
        enabled: true,
      },
    ]);
    setDraft(null);
  }, [draft, replacements, persist, t]);

  const updateRow = useCallback(
    (
      id: string,
      patch: Partial<{
        find: string;
        replace: string;
        enabled: boolean;
      }>,
    ) => {
      persist(replacements.map((r) => (r.id === id ? { ...r, ...patch } : r)));
    },
    [replacements, persist],
  );

  const removeRow = useCallback(
    (id: string) => {
      persist(replacements.filter((r) => r.id !== id));
    },
    [replacements, persist],
  );

  return (
    <div
      className={
        grouped ? "w-full" : "p-4 border border-mid-gray/20 rounded-lg"
      }
    >
      <div className="flex items-center justify-between gap-2 mb-2">
        <div>
          <p className="text-sm font-medium">
            {t("settings.advanced.textReplacements.label")}
          </p>
          {descriptionMode === "inline" && (
            <p className="text-xs text-mid-gray mt-0.5">
              {t("settings.advanced.textReplacements.description")}
            </p>
          )}
        </div>
        <button
          onClick={() => setDraft({ find: "", replace: "" })}
          disabled={isUpdating("text_replacements") || draft !== null}
          className="flex items-center gap-1 px-2 py-1 rounded-md bg-logo-primary/80 hover:bg-logo-primary text-xs font-medium disabled:opacity-50"
        >
          <Plus size={14} />
          {t("settings.advanced.textReplacements.add")}
        </button>
      </div>

      {descriptionMode === "tooltip" && (
        <p className="text-xs text-mid-gray mb-2">
          {t("settings.advanced.textReplacements.description")}
        </p>
      )}

      <div className="space-y-2">
        {replacements.map((r) => (
          <div key={r.id} className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={r.enabled}
              onChange={(e) => updateRow(r.id, { enabled: e.target.checked })}
              className="shrink-0"
              title={t("settings.advanced.textReplacements.enabled")}
            />
            <input
              value={r.find}
              onChange={(e) => updateRow(r.id, { find: e.target.value })}
              placeholder={t(
                "settings.advanced.textReplacements.findPlaceholder",
              )}
              className="flex-1 px-2 py-1 rounded-md bg-mid-gray/10 border border-mid-gray/20 text-sm min-w-0"
            />
            <span className="text-mid-gray shrink-0">→</span>
            <input
              value={r.replace}
              onChange={(e) => updateRow(r.id, { replace: e.target.value })}
              placeholder={t(
                "settings.advanced.textReplacements.replacePlaceholder",
              )}
              className="flex-1 px-2 py-1 rounded-md bg-mid-gray/10 border border-mid-gray/20 text-sm min-w-0"
            />
            <button
              onClick={() => removeRow(r.id)}
              disabled={isUpdating("text_replacements")}
              className="text-mid-gray hover:text-red-400 shrink-0 disabled:opacity-50"
              title={t("settings.advanced.textReplacements.remove")}
            >
              <Trash2 size={16} />
            </button>
          </div>
        ))}

        {draft && (
          <div className="flex items-center gap-2">
            <span className="w-4 shrink-0" />
            <input
              autoFocus
              value={draft.find}
              onChange={(e) => setDraft({ ...draft, find: e.target.value })}
              onKeyDown={(e) => e.key === "Enter" && confirmAdd()}
              placeholder={t(
                "settings.advanced.textReplacements.findPlaceholder",
              )}
              className="flex-1 px-2 py-1 rounded-md bg-mid-gray/10 border border-mid-gray/20 text-sm min-w-0"
            />
            <span className="text-mid-gray shrink-0">→</span>
            <input
              value={draft.replace}
              onChange={(e) => setDraft({ ...draft, replace: e.target.value })}
              onKeyDown={(e) => e.key === "Enter" && confirmAdd()}
              placeholder={t(
                "settings.advanced.textReplacements.replacePlaceholder",
              )}
              className="flex-1 px-2 py-1 rounded-md bg-mid-gray/10 border border-mid-gray/20 text-sm min-w-0"
            />
            <button
              onClick={confirmAdd}
              disabled={isUpdating("text_replacements")}
              className="text-sm font-medium text-logo-primary shrink-0 disabled:opacity-50"
            >
              {t("settings.advanced.textReplacements.save")}
            </button>
          </div>
        )}

        {replacements.length === 0 && !draft && (
          <p className="text-xs text-mid-gray/70">
            {t("settings.advanced.textReplacements.empty")}
          </p>
        )}

        {error && <p className="text-xs text-red-400">{error}</p>}
      </div>
    </div>
  );
};
