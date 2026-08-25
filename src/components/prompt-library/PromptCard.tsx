import React, { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Clipboard, Copy, Edit, Star, Trash2 } from "lucide-react";
import { toast } from "sonner";
import type { Prompt } from "@/bindings";
import { commands } from "@/bindings";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { VariableFillDialog } from "./VariableFillDialog";

interface Props {
  prompt: Prompt;
  onEdit: (p: Prompt) => void;
  onRefresh: () => void;
  /** Active FTS query for safe highlight (never innerHTML). */
  highlightQuery?: string;
}

/**
 * Safe highlight: split `text` on `query` terms and wrap matches in
 * `<mark>`. Mirrors `RecordingOverlay.tsx:206 committedNodes` — builds
 * `ReactNode[]` and never uses `innerHTML`/`dangerouslySetInnerHTML`, so
 * checker/FTS output has no XSS surface. Case-insensitive, diacritics
 * already stripped by `unicode61 "remove_diacritics 1"` on the SQLite side.
 */
const highlightNodes = (text: string, query?: string): React.ReactNode[] => {
  if (!query?.trim() || !text) return [text];
  const terms = query
    .trim()
    .split(/\s+/)
    .map((t) => t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
    .filter(Boolean);
  if (terms.length === 0) return [text];
  const pattern = terms.join("|");
  const splitRe = new RegExp(`(${pattern})`, "gi");
  const testRe = new RegExp(`^(${pattern})$`, "i");
  const parts = text.split(splitRe);
  return parts.map((part, i) =>
    testRe.test(part) ? (
      <mark key={i} className="bg-amber-300/40 rounded px-0.5">
        {part}
      </mark>
    ) : (
      <React.Fragment key={i}>{part}</React.Fragment>
    ),
  );
};

const highlightNodesWithRegex = (
  text: string,
  splitRe: RegExp | null,
  testRe: RegExp | null,
): React.ReactNode[] => {
  if (!splitRe || !testRe || !text) return [text];
  const parts = text.split(splitRe);
  return parts.map((part, i) =>
    testRe.test(part) ? (
      <mark key={i} className="bg-amber-300/40 rounded px-0.5">
        {part}
      </mark>
    ) : (
      <React.Fragment key={i}>{part}</React.Fragment>
    ),
  );
};

export const PromptCard: React.FC<Props> = ({ prompt, onEdit, onRefresh, highlightQuery }) => {
  const { t } = useTranslation();
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [showVars, setShowVars] = useState(false);

  const { splitRe, testRe } = useMemo(() => {
    if (!highlightQuery?.trim()) return { splitRe: null as RegExp | null, testRe: null as RegExp | null };
    const terms = highlightQuery
      .trim()
      .split(/\s+/)
      .map((term) => term.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
      .filter(Boolean);
    if (terms.length === 0) return { splitRe: null as RegExp | null, testRe: null as RegExp | null };
    const pattern = terms.join("|");
    return {
      splitRe: new RegExp(`(${pattern})`, "gi"),
      testRe: new RegExp(`^(${pattern})$`, "i"),
    };
  }, [highlightQuery]);

  const titleNodes = useMemo(() => highlightNodesWithRegex(prompt.title, splitRe, testRe), [prompt.title, splitRe, testRe]);
  const contentNodes = useMemo(() => highlightNodesWithRegex(prompt.content, splitRe, testRe), [prompt.content, splitRe, testRe]);

  const handleInsert = async (vars?: Record<string, string>) => {
    const res = await commands.insertPrompt(prompt.id, vars ?? null);
    if (res.status === "error") {
      toast.error(res.error);
    } else {
      toast.success(t("promptLibrary.inserted"));
    }
  };

  const onInsertClick = () => {
    if (prompt.variables && prompt.variables.length > 0) {
      setShowVars(true);
    } else {
      handleInsert();
    }
  };

  const handleDelete = async () => {
    const res = await commands.deletePrompt(prompt.id);
    if (res.status === "error") toast.error(res.error);
    else onRefresh();
    setConfirmDelete(false);
  };

  const handleDuplicate = async () => {
    const res = await commands.duplicatePrompt(prompt.id);
    if (res.status === "error") toast.error(res.error);
    else onRefresh();
  };

  const handleTogglePin = async () => {
    const res = await commands.togglePromptPin(prompt.id);
    if (res.status === "error") toast.error(res.error);
    else onRefresh();
  };

  const handleCopy = async () => {
    await navigator.clipboard.writeText(prompt.content);
    toast.success(t("common.copied"));
  };

  return (
    <div className="p-3 rounded-lg border border-mid-gray/20 bg-mid-gray/5 space-y-2">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <p className="text-sm font-medium truncate">{titleNodes}</p>
          <p className="text-xs text-text/60 line-clamp-2 whitespace-pre-wrap break-words">
            {contentNodes}
          </p>
          <div className="flex flex-wrap gap-1 mt-1">
            {prompt.folder_name && (
              <span className="text-[10px] px-1.5 py-0.5 rounded bg-mid-gray/20">
                {prompt.folder_name}
              </span>
            )}
            {prompt.tags.map((tag) => (
              <span key={tag} className="text-[10px] px-1.5 py-0.5 rounded bg-logo-primary/20">
                {tag}
              </span>
            ))}
            {prompt.variables.length > 0 && (
              <span className="text-[10px] px-1.5 py-0.5 rounded bg-amber-500/20">
                {prompt.variables.join(", ")}
              </span>
            )}
          </div>
        </div>
        <button
          onClick={handleTogglePin}
          className={`p-1 rounded ${prompt.pinned ? "text-amber-500" : "text-text/40 hover:text-text"}`}
          title={t("promptLibrary.pin")}
        >
          <Star className={`w-4 h-4 ${prompt.pinned ? "fill-amber-500" : ""}`} />
        </button>
      </div>
      <div className="flex items-center gap-1 text-xs text-text/60">
        <span>
          {t("promptLibrary.usageCount", { count: prompt.usage_count })}
        </span>
        {/* eslint-disable-next-line i18next/no-literal-string */}
        <span>· v{prompt.version}</span>
      </div>
      <div className="flex flex-wrap gap-1">
        <Button variant="secondary" size="sm" onClick={onInsertClick}>
          <Clipboard className="w-3.5 h-3.5" />
          {t("promptLibrary.insert")}
        </Button>
        <Button variant="ghost" size="sm" onClick={handleCopy}>
          <Copy className="w-3.5 h-3.5" />
          {t("common.copy")}
        </Button>
        <Button variant="ghost" size="sm" onClick={() => onEdit(prompt)}>
          <Edit className="w-3.5 h-3.5" />
          {t("common.edit")}
        </Button>
        <Button variant="ghost" size="sm" onClick={handleDuplicate}>
          <Copy className="w-3.5 h-3.5" />
          {t("promptLibrary.duplicate")}
        </Button>
        <Button variant="ghost" size="sm" onClick={() => setConfirmDelete(true)}>
          <Trash2 className="w-3.5 h-3.5" />
          {t("common.delete")}
        </Button>
      </div>
      <Dialog
        open={confirmDelete}
        onOpenChange={setConfirmDelete}
        title={t("promptLibrary.deleteConfirmTitle")}
        closeLabel={t("common.close")}
        footer={
          <>
            <Button variant="secondary" size="sm" onClick={() => setConfirmDelete(false)}>
              {t("common.cancel")}
            </Button>
            <Button variant="danger" size="sm" onClick={handleDelete}>
              {t("common.delete")}
            </Button>
          </>
        }
      >
        <p className="text-sm">{t("promptLibrary.deleteConfirm", { title: prompt.title })}</p>
      </Dialog>
      <VariableFillDialog
        open={showVars}
        onOpenChange={setShowVars}
        prompt={prompt}
        onSubmit={handleInsert}
      />
    </div>
  );
};
