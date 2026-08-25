import React, { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Clipboard, Search, Trash2, Send, Eraser } from "lucide-react";
import { toast } from "sonner";
import { commands, events } from "@/bindings";
import type { PromptHistoryEntry } from "@/bindings";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { formatDate, formatDateTime } from "@/utils/dateFormat";
import { usePromptDraftStore } from "@/stores/promptDraftStore";
import type { AppSection } from "@/lib/types/navigation";

function groupByDay(entries: PromptHistoryEntry[], locale: string): Map<string, PromptHistoryEntry[]> {
  const map = new Map<string, PromptHistoryEntry[]>();
  for (const e of entries) {
    const d = new Date(e.timestamp * 1000);
    const key = d.toISOString().slice(0, 10);
    const arr = map.get(key);
    if (arr) arr.push(e);
    else map.set(key, [e]);
  }
  // ensure newest day first (entries already newest-first)
  return map;
}

export const HistoryTimeline: React.FC<{ onNavigate?: (s: AppSection) => void }> = ({ onNavigate }) => {
  const { t, i18n } = useTranslation();
  const locale = i18n.language;
  const [entries, setEntries] = useState<PromptHistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState("");
  const [clearOpen, setClearOpen] = useState(false);

  useEffect(() => {
    let cancelled = false;
    commands
      .listPromptHistory(100)
      .then((res) => {
        if (cancelled) return;
        if (res.status === "ok") setEntries(res.data);
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const unlisten = events.promptHistoryUpdatePayload.listen((ev) => {
      const p: any = ev.payload;
      if (p.action === "added" && p.entry) setEntries((prev) => [p.entry, ...prev].slice(0, 100));
      else if (p.action === "deleted" && p.id) setEntries((prev) => prev.filter((e) => e.id !== p.id));
      else if (p.action === "cleared") setEntries([]);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return entries;
    return entries.filter((e) => e.prompt_text.toLowerCase().includes(q));
  }, [entries, search]);

  const groups = useMemo(() => groupByDay(filtered, locale), [filtered, locale]);

  const handleReuse = (e: PromptHistoryEntry) => {
    usePromptDraftStore.getState().setDraft(e.prompt_text);
    onNavigate?.("home");
    toast.success(t("promptStudio.historyReuse"));
  };

  const handleClear = async () => {
    setClearOpen(false);
    const res = await commands.clearPromptHistory();
    if (res.status === "error") toast.error(t("home.history.clearFailed"));
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-4">
      <div>
        <h1 className="text-xl font-semibold">{t("home.history.title")}</h1>
        <p className="text-sm text-text/60">{t("home.history.description")}</p>
      </div>

      <div className="flex gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-text/40 pointer-events-none" />
          <input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={t("home.history.searchPlaceholder")}
            className="w-full pl-9 pr-3 py-2 text-sm bg-mid-gray/10 border border-mid-gray/20 rounded-lg focus:outline-none focus:ring-1 focus:ring-logo-primary placeholder:text-text/40"
          />
        </div>
        {entries.length > 0 && (
          <Button variant="ghost" size="sm" onClick={() => setClearOpen(true)}>
            <Eraser className="w-3.5 h-3.5" />
            {t("home.history.clear")}
          </Button>
        )}
      </div>

      {loading ? (
        <p className="text-sm text-text/50 text-center py-8">{t("home.history.loading")}</p>
      ) : filtered.length === 0 ? (
        <p className="text-sm text-text/50 text-center py-8">
          {search ? t("home.history.noResults") : t("home.history.empty")}
        </p>
      ) : (
        <div className="space-y-6">
          {Array.from(groups.entries()).map(([dayKey, dayEntries]) => {
            const dayTs = String(Math.floor(new Date(dayKey + "T12:00:00Z").getTime() / 1000));
            return (
              <div key={dayKey} className="space-y-2">
                <div className="flex items-center gap-2">
                  <div className="h-px flex-1 bg-mid-gray/15" />
                  <span className="text-xs font-semibold tracking-widest uppercase text-text/40 px-2">
                    {formatDate(dayTs, locale)} · {dayEntries.length}
                  </span>
                  <div className="h-px flex-1 bg-mid-gray/15" />
                </div>
                <div className="rounded-xl border border-mid-gray/15 bg-background overflow-hidden divide-y divide-mid-gray/10">
                  {dayEntries.map((e) => (
                    <div key={e.id} className="p-3 flex gap-3">
                      <div className="min-w-0 flex-1 space-y-1">
                        <p className="text-xs text-text/45">{formatDateTime(String(e.timestamp), locale)}</p>
                        <p className="text-sm whitespace-pre-wrap break-words leading-relaxed">{e.prompt_text}</p>
                      </div>
                      <div className="flex flex-col gap-1 shrink-0">
                        <button
                          onClick={async () => {
                            await navigator.clipboard.writeText(e.prompt_text);
                            toast.success(t("common.copied"));
                          }}
                          title={t("home.history.copy")}
                          className="w-8 h-8 grid place-items-center rounded-md hover:bg-mid-gray/10 text-text/50 hover:text-text"
                        >
                          <Clipboard className="w-3.5 h-3.5" />
                        </button>
                        <button
                          onClick={() => handleReuse(e)}
                          title={t("home.history.reuse")}
                          className="w-8 h-8 grid place-items-center rounded-md hover:bg-mid-gray/10 text-text/50 hover:text-text"
                        >
                          <Send className="w-3.5 h-3.5 rotate-180" />
                        </button>
                        <button
                          onClick={async () => {
                            const r = await commands.deletePromptHistoryEntry(e.id);
                            if (r.status === "error") toast.error(t("home.history.deleteFailed"));
                          }}
                          title={t("home.history.delete")}
                          className="w-8 h-8 grid place-items-center rounded-md hover:bg-mid-gray/10 text-text/40 hover:text-red-500"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            );
          })}
        </div>
      )}

      <Dialog
        open={clearOpen}
        onOpenChange={setClearOpen}
        title={t("home.history.clearTitle")}
        closeLabel={t("common.close")}
        footer={
          <>
            <Button variant="secondary" size="sm" onClick={() => setClearOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button variant="danger" size="sm" onClick={handleClear}>
              {t("common.delete")}
            </Button>
          </>
        }
      >
        <p className="text-sm text-text/70">{t("home.history.clearConfirm", { count: entries.length })}</p>
      </Dialog>
    </div>
  );
};
