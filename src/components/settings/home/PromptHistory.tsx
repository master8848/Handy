import React, { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Clipboard, Search, Send, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { commands, events } from "@/bindings";
import type { PromptHistoryEntry } from "@/bindings";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { formatDateTime } from "@/utils/dateFormat";
import { usePromptDraftStore } from "@/stores/promptDraftStore";
import type { SettingsSectionProps } from "@/lib/types/navigation";

export const PromptHistory: React.FC<SettingsSectionProps> = ({
  onNavigate,
}) => {
  const { t, i18n } = useTranslation();
  const [entries, setEntries] = useState<PromptHistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState("");
  const [clearConfirmOpen, setClearConfirmOpen] = useState(false);

  // Load pasted prompts and keep them live via backend events.
  useEffect(() => {
    let cancelled = false;
    commands
      .listPromptHistory(null)
      .then((res) => {
        if (cancelled) return;
        if (res.status === "ok") setEntries(res.data);
      })
      .catch((err) => console.error("Failed to load prompt history:", err))
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const unlisten = events.promptHistoryUpdatePayload.listen((event) => {
      const payload = event.payload;
      if (payload.action === "added") {
        setEntries((prev) => [payload.entry, ...prev]);
      } else if (payload.action === "deleted") {
        setEntries((prev) => prev.filter((e) => e.id !== payload.id));
      } else {
        setEntries([]);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const filteredEntries = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!query) return entries;
    return entries.filter((entry) =>
      entry.prompt_text.toLowerCase().includes(query),
    );
  }, [entries, search]);

  const handleDelete = async (id: number) => {
    const result = await commands.deletePromptHistoryEntry(id);
    if (result.status === "error") {
      toast.error(t("home.history.deleteFailed"));
    }
  };

  const handleClear = async () => {
    setClearConfirmOpen(false);
    const result = await commands.clearPromptHistory();
    if (result.status === "error") {
      toast.error(t("home.history.clearFailed"));
    }
  };

  const handleReuse = (entry: PromptHistoryEntry) => {
    usePromptDraftStore.getState().setDraft(entry.prompt_text);
    onNavigate?.("home");
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-4">
      <div className="mb-2">
        <h1 className="text-xl font-semibold">{t("home.history.title")}</h1>
        <p className="text-sm text-text/60">{t("home.history.description")}</p>
      </div>

      {/* Search — filter pasted prompts by text */}
      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-text/40 pointer-events-none" />
        <input
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder={t("home.history.searchPlaceholder")}
          className="w-full pl-9 pr-3 py-2 text-sm bg-mid-gray/10 border border-mid-gray/40 rounded-lg focus:outline-none focus:ring-1 focus:ring-logo-primary placeholder:text-text/40"
        />
      </div>

      {entries.length > 0 && (
        <div className="flex justify-end">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setClearConfirmOpen(true)}
          >
            <Trash2 className="w-3.5 h-3.5" />
            {t("home.history.clear")}
          </Button>
        </div>
      )}

      {loading ? (
        <div className="px-4 py-3 text-center text-sm text-text/60">
          {t("home.history.loading")}
        </div>
      ) : filteredEntries.length === 0 ? (
        <div className="px-4 py-3 text-center text-sm text-text/60">
          {search ? t("home.history.noResults") : t("home.history.empty")}
        </div>
      ) : (
        <div className="divide-y divide-mid-gray/20 rounded-md border border-mid-gray/20 bg-mid-gray/5">
          {filteredEntries.map((entry) => (
            <div key={entry.id} className="p-3 space-y-2">
              <div className="flex items-center justify-between gap-2">
                <span className="text-xs text-text/60">
                  {formatDateTime(String(entry.timestamp), i18n.language)}
                </span>
                <div className="flex items-center gap-1">
                  <button
                    title={t("home.history.copy")}
                    onClick={async () => {
                      await navigator.clipboard.writeText(entry.prompt_text);
                      toast.success(t("common.copied"));
                    }}
                    className="p-1.5 rounded hover:bg-mid-gray/20 text-text/70 hover:text-text"
                  >
                    <Clipboard className="w-3.5 h-3.5" />
                  </button>
                  <button
                    title={t("home.history.reuse")}
                    onClick={() => handleReuse(entry)}
                    className="p-1.5 rounded hover:bg-mid-gray/20 text-text/70 hover:text-text"
                  >
                    <Send className="w-3.5 h-3.5 rotate-180" />
                  </button>
                  <button
                    title={t("home.history.delete")}
                    onClick={() => handleDelete(entry.id)}
                    className="p-1.5 rounded hover:bg-mid-gray/20 text-text/70 hover:text-red-400"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>
                </div>
              </div>
              <p className="text-sm whitespace-pre-wrap break-words">
                {entry.prompt_text}
              </p>
            </div>
          ))}
        </div>
      )}

      <Dialog
        open={clearConfirmOpen}
        onOpenChange={setClearConfirmOpen}
        title={t("home.history.clearTitle")}
        closeLabel={t("common.close")}
        footer={
          <>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => setClearConfirmOpen(false)}
            >
              {t("common.cancel")}
            </Button>
            <Button variant="danger" size="sm" onClick={handleClear}>
              {t("common.delete")}
            </Button>
          </>
        }
      >
        <p className="text-sm text-text/70">
          {t("home.history.clearConfirm", { count: entries.length })}
        </p>
      </Dialog>
    </div>
  );
};
