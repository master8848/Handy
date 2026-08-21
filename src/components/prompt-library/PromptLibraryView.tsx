import React, { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Search, Plus, Folder as FolderIcon, Download, Upload } from "lucide-react";
import { toast } from "sonner";
import { commands } from "@/bindings";
import type { Prompt } from "@/bindings";
import { Button } from "@/components/ui/Button";
import { usePromptLibraryStore } from "@/stores/promptLibraryStore";
import { PromptCard } from "./PromptCard";
import { PromptEditor } from "./PromptEditor";
import { FolderManager } from "./FolderManager";
import { VersionHistoryDrawer } from "./VersionHistoryDrawer";
import { SEARCH_DEBOUNCE_MS } from "@/lib/constants/debounce";

export const PromptLibraryView: React.FC = () => {
  const { t } = useTranslation();
  const { prompts, folders, tags, search, selectedFolderId, selectedTag, sort, setSearch, setSelectedFolderId, setSelectedTag, setSort, refresh, refreshFolders, refreshTags } = usePromptLibraryStore();
  const [debounced, setDebounced] = useState(search);
  const [editorOpen, setEditorOpen] = useState(false);
  const [editing, setEditing] = useState<Prompt | null>(null);
  const [folderMgrOpen, setFolderMgrOpen] = useState(false);
  const [versionPromptId, setVersionPromptId] = useState<number | null>(null);

  useEffect(() => {
    refresh();
    refreshFolders();
    refreshTags();
  }, [refresh, refreshFolders, refreshTags]);

  // FTS-backed search: 150 ms debounce (faster than the 300 ms
  // spell-check debounce) so typing feels instant while still throttling
  // IPC. See `src/lib/constants/debounce.ts`.
  useEffect(() => {
    const id = setTimeout(() => setDebounced(search), SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(id);
  }, [search]);

  useEffect(() => {
    refresh();
  }, [debounced, selectedFolderId, selectedTag, sort]);

  const filtered = useMemo(() => prompts, [prompts]);

  const handleExport = async () => {
    const res = await commands.exportPrompts();
    if (res.status === "error") toast.error(res.error);
    else {
      await navigator.clipboard.writeText(res.data);
      toast.success(t("promptLibrary.exported"));
    }
  };

  const handleImport = async () => {
    const text = await navigator.clipboard.readText().catch(() => "");
    if (!text) {
      toast.error(t("promptLibrary.importFailed"));
      return;
    }
    const res = await commands.importPrompts(text);
    if (res.status === "error") toast.error(res.error);
    else {
      toast.success(t("promptLibrary.imported", { count: res.data }));
      refresh();
    }
  };

  return (
    <div className="max-w-4xl w-full mx-auto space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold">{t("promptLibrary.title")}</h1>
          <p className="text-sm text-text/60">{t("promptLibrary.description")}</p>
        </div>
        <div className="flex gap-2">
          <Button size="sm" onClick={() => { setEditing(null); setEditorOpen(true); }}>
            <Plus className="w-4 h-4" />
            {t("promptLibrary.newPrompt")}
          </Button>
          <Button variant="ghost" size="sm" onClick={() => setFolderMgrOpen(true)}>
            <FolderIcon className="w-4 h-4" />
            {t("promptLibrary.folders")}
          </Button>
        </div>
      </div>

      <div className="flex flex-wrap gap-2 items-center">
        <div className="relative flex-1 min-w-[200px]">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-text/40 pointer-events-none" />
          <input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={t("promptLibrary.searchPlaceholder")}
            className="w-full pl-9 pr-3 py-2 text-sm bg-mid-gray/10 border border-mid-gray/40 rounded-lg focus:outline-none focus:ring-1 focus:ring-logo-primary"
          />
        </div>
        <select value={String(selectedFolderId ?? "")} onChange={(e) => setSelectedFolderId(e.target.value ? Number(e.target.value) : null)} className="px-2 py-2 text-sm bg-mid-gray/10 border border-mid-gray/40 rounded-lg">
          <option value="">{t("promptLibrary.allFolders")}</option>
          {folders.map((f) => (
            <option key={f.id} value={String(f.id)}>
              {f.name}
            </option>
          ))}
        </select>
        <select value={selectedTag ?? ""} onChange={(e) => setSelectedTag(e.target.value || null)} className="px-2 py-2 text-sm bg-mid-gray/10 border border-mid-gray/40 rounded-lg">
          <option value="">{t("promptLibrary.allTags")}</option>
          {tags.map((tag) => (
            <option key={tag.id} value={tag.name}>
              {tag.name}
            </option>
          ))}
        </select>
        <select value={sort} onChange={(e) => setSort(e.target.value)} className="px-2 py-2 text-sm bg-mid-gray/10 border border-mid-gray/40 rounded-lg">
          <option value="updatedDesc">{t("promptLibrary.sortUpdated")}</option>
          <option value="createdDesc">{t("promptLibrary.sortCreated")}</option>
          <option value="usageDesc">{t("promptLibrary.sortUsage")}</option>
          <option value="titleAsc">{t("promptLibrary.sortTitle")}</option>
        </select>
      </div>

      <div className="flex gap-2">
        <Button variant="ghost" size="sm" onClick={handleExport}>
          <Download className="w-3.5 h-3.5" />
          {t("promptLibrary.export")}
        </Button>
        <Button variant="ghost" size="sm" onClick={handleImport}>
          <Upload className="w-3.5 h-3.5" />
          {t("promptLibrary.import")}
        </Button>
        <Button variant="ghost" size="sm" onClick={() => setVersionPromptId(filtered[0]?.id ?? null)} disabled={filtered.length === 0}>
          {t("promptLibrary.versionHistory")}
        </Button>
      </div>

      {filtered.length === 0 ? (
        <p className="text-sm text-text/60 text-center py-8">{t("promptLibrary.empty")}</p>
      ) : (
        <div className="grid gap-3">
          {filtered.map((p) => (
            <PromptCard
              key={p.id}
              prompt={p}
              highlightQuery={debounced}
              onEdit={(pr) => { setEditing(pr); setEditorOpen(true); }}
              onRefresh={refresh}
            />
          ))}
        </div>
      )}

      <PromptEditor open={editorOpen} onOpenChange={setEditorOpen} prompt={editing} folders={folders} onSaved={refresh} />
      <FolderManager open={folderMgrOpen} onOpenChange={setFolderMgrOpen} folders={folders} onChanged={() => { refreshFolders(); refresh(); }} />
      <VersionHistoryDrawer open={versionPromptId !== null} onOpenChange={(o) => !o && setVersionPromptId(null)} promptId={versionPromptId} onRestored={refresh} />
    </div>
  );
};
