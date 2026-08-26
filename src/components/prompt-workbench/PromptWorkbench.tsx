import React, { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import Placeholder from "@tiptap/extension-placeholder";
import {
  ChevronDown,
  ChevronUp,
  Clipboard,
  Copy,
  Ellipsis,
  Eraser,
  ExternalLink,
  Loader2,
  Mic,
  PenLine,
  Search,
  Send,
} from "lucide-react";
import { toast } from "sonner";
import { commands, events } from "@/bindings";
import type { Prompt, PromptHistoryEntry, SpellingIssue } from "@/bindings";
import { useSettings } from "@/hooks/useSettings";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { PromptFormatToolbar } from "@/components/shared";
import { useWorkbenchDraftStore } from "@/stores/workbenchDraftStore";
import { usePromptDraftStore } from "@/stores/promptDraftStore";
import { usePromptLibraryStore } from "@/stores/promptLibraryStore";
import { VariableFillDialog } from "@/components/prompt-library/VariableFillDialog";
import { PromptEditor } from "@/components/prompt-library/PromptEditor";
import { PromptLibraryView } from "@/components/prompt-library/PromptLibraryView";
import { VariableChipExtension } from "./variableChipExtension";
import {
  positionsFromOffsets,
  spellIssueCache,
  SpellCheckExtension,
  SPELL_CHECK_META,
} from "@/components/settings/home/spellCheckExtension";
import { SPELL_CHECK_DEBOUNCE_MS, SEARCH_DEBOUNCE_MS } from "@/lib/constants/debounce";
import { formatDateTime } from "@/utils/dateFormat";

const SPELL_HOVER_DELAY_MS = 250;
const PROMPT_AUTO_SAVE_DEBOUNCE_MS = 1500;

type DictationPhase = "idle" | "recording" | "transcribing";

const formatElapsed = (seconds: number): string => {
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}:${String(secs).padStart(2, "0")}`;
};

// Compact studio card — Insert/Copy primary, rest in overflow menu
const StudioPromptCard: React.FC<{
  prompt: Prompt;
  onEdit: (p: Prompt) => void;
  onRefresh: () => void;
  onInsertToComposer?: (text: string) => void;
}> = ({ prompt, onEdit, onRefresh, onInsertToComposer }) => {
  const { t } = useTranslation();
  const [showVars, setShowVars] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!menuOpen) return;
    const onDoc = (e: MouseEvent) => {
      if (!menuRef.current?.contains(e.target as Node)) setMenuOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [menuOpen]);

  const handleInsert = async (vars?: Record<string, string>) => {
    const res = await commands.insertPrompt(prompt.id, vars ?? null);
    if (res.status === "error") toast.error(res.error);
    else toast.success(t("promptLibrary.inserted"));
  };
  const onInsertClick = () => {
    if (prompt.variables.length > 0) setShowVars(true);
    else handleInsert();
  };
  const handleCopy = async () => {
    await navigator.clipboard.writeText(prompt.content);
    toast.success(t("promptStudio.copied"));
  };
  const handleDuplicate = async () => {
    const res = await commands.duplicatePrompt(prompt.id);
    if (res.status === "error") toast.error(res.error);
    else onRefresh();
    setMenuOpen(false);
  };
  const handleDelete = async () => {
    const res = await commands.deletePrompt(prompt.id);
    if (res.status === "error") toast.error(res.error);
    else onRefresh();
    setConfirmDelete(false);
    setMenuOpen(false);
  };
  const handleTogglePin = async () => {
    const res = await commands.togglePromptPin(prompt.id);
    if (res.status === "error") toast.error(res.error);
    else onRefresh();
  };

  return (
    <div className="flex flex-col p-3 rounded-xl border border-mid-gray/15 bg-mid-gray/[0.04] hover:bg-mid-gray/[0.07] hover:border-mid-gray/25 transition-colors min-h-[116px]">
      <div className="flex items-start justify-between gap-2 min-w-0">
        <p className="text-[13px] font-medium leading-tight truncate flex-1 min-w-0">{prompt.title}</p>
        <button
          onClick={handleTogglePin}
          title={t("promptLibrary.pin")}
          className={`shrink-0 p-1 rounded-md -mt-1 -me-1 ${prompt.pinned ? "text-amber-500" : "text-text/30 hover:text-text/70"}`}
        >
          <span className={`block w-2 h-2 rounded-full ${prompt.pinned ? "bg-amber-500" : "bg-text/20"}`} aria-hidden />
        </button>
      </div>
      <p className="text-xs text-text/55 line-clamp-2 whitespace-pre-wrap break-words mt-1 flex-1">
        {prompt.content}
      </p>
      {(prompt.folder_name || prompt.tags.length > 0 || prompt.variables.length > 0) && (
        <div className="flex flex-wrap gap-1 mt-2">
          {prompt.folder_name && (
            <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-mid-gray/15 text-text/60">{prompt.folder_name}</span>
          )}
          {prompt.tags.slice(0, 2).map((tag) => (
            <span key={tag} className="text-[10px] px-1.5 py-0.5 rounded-full bg-logo-primary/15 text-logo-primary">
              {tag}
            </span>
          ))}
          {prompt.variables.length > 0 && (
            <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-amber-500/15 text-amber-600">
              {prompt.variables.slice(0, 2).join(", ")}
              {prompt.variables.length > 2 ? " +" + (prompt.variables.length - 2) : ""}
            </span>
          )}
        </div>
      )}
      <div className="flex items-center gap-1 mt-3">
        <Button variant="secondary" size="sm" onClick={onInsertClick} className="flex-1 justify-center">
          <Send className="w-3 h-3" />
          {t("promptStudio.insert")}
        </Button>
        <Button variant="ghost" size="sm" onClick={handleCopy} title={t("common.copy")} aria-label={t("common.copy")} className="px-2">
          <Copy className="w-3.5 h-3.5" />
        </Button>
        <div className="relative" ref={menuRef}>
          <Button variant="ghost" size="sm" onClick={() => setMenuOpen((v) => !v)} aria-label="More" className="px-2">
            <Ellipsis className="w-3.5 h-3.5" />
          </Button>
          {menuOpen && (
            <div className="absolute right-0 bottom-full mb-1 w-36 rounded-lg border border-mid-gray/20 bg-background shadow-lg py-1 z-20">
              <button onClick={() => { setMenuOpen(false); onEdit(prompt); }} className="w-full text-left px-3 py-1.5 text-xs hover:bg-mid-gray/10 flex items-center gap-2">
                <PenLine className="w-3 h-3" /> {t("common.edit")}
              </button>
              <button onClick={() => { if (onInsertToComposer) onInsertToComposer(prompt.content); setMenuOpen(false); }} className="w-full text-left px-3 py-1.5 text-xs hover:bg-mid-gray/10 flex items-center gap-2">
                <Clipboard className="w-3 h-3" /> {t("promptStudio.copy")}
              </button>
              <button onClick={handleDuplicate} className="w-full text-left px-3 py-1.5 text-xs hover:bg-mid-gray/10 flex items-center gap-2">
                <Copy className="w-3 h-3" /> {t("promptLibrary.duplicate")}
              </button>
              <button onClick={() => setConfirmDelete(true)} className="w-full text-left px-3 py-1.5 text-xs hover:bg-mid-gray/10 text-red-500 flex items-center gap-2">
                <Eraser className="w-3 h-3" /> {t("common.delete")}
              </button>
            </div>
          )}
        </div>
      </div>
      <Dialog
        open={confirmDelete}
        onOpenChange={setConfirmDelete}
        title={t("promptLibrary.deleteConfirmTitle")}
        closeLabel={t("common.close")}
        footer={
          <>
            <Button variant="secondary" size="sm" onClick={() => setConfirmDelete(false)}>{t("common.cancel")}</Button>
            <Button variant="danger" size="sm" onClick={handleDelete}>{t("common.delete")}</Button>
          </>
        }
      >
        <p className="text-sm">{t("promptLibrary.deleteConfirm", { title: prompt.title })}</p>
      </Dialog>
      <VariableFillDialog open={showVars} onOpenChange={setShowVars} prompt={prompt} onSubmit={handleInsert} />
    </div>
  );
};

/**
 * Prompt Studio — the "Prompt" persona.
 * Composer on top, quick filters + compact library grid inline, Send bar, and a thin history strip.
 * The full PromptLibraryView stays mounted behind the "Show all" expand — no separate window needed.
 */
export const PromptWorkbench: React.FC = () => {
  const { t, i18n } = useTranslation();
  const { settings, updateSetting } = useSettings();

  const [phase, setPhase] = useState<DictationPhase>("idle");
  const [error, setError] = useState<string | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const [pasting, setPasting] = useState(false);
  const [docText, setDocText] = useState("");
  const [activeIssue, setActiveIssue] = useState<SpellingIssue | null>(null);
  const [popoverPos, setPopoverPos] = useState<{ left: number; top: number; above: boolean } | null>(null);
  const [libraryExpanded, setLibraryExpanded] = useState(false);
  const [gridLimit, setGridLimit] = useState(6);
  const [historyCollapsed, setHistoryCollapsed] = useState(false);
  const [historyEntries, setHistoryEntries] = useState<PromptHistoryEntry[]>([]);
  const [editorOpen, setEditorOpen] = useState(false);
  const [editing, setEditing] = useState<Prompt | null>(null);

  const spellEnabled = settings?.spell_check_enabled ?? true;
  const extensions = useMemo(
    () => [
      StarterKit,
      Placeholder.configure({ placeholder: t("promptStudio.composerPlaceholder") }),
      SpellCheckExtension,
      VariableChipExtension,
    ],
    [t],
  );

  const editor = useEditor({
    extensions,
    content: "",
    editorProps: {
      attributes: { class: "ptap-body ptap-workbench !min-h-[160px]", spellcheck: "false" },
    },
    onUpdate: ({ editor }) => {
      const text = editor.state.doc.textContent;
      setDocText(text);
      useWorkbenchDraftStore.getState().setDraftHtml(editor.getHTML());
    },
    onTransaction: () => setEditorVersion((v) => v + 1),
  });

  const [editorVersion, setEditorVersion] = useState(0);
  void editorVersion;

  const popoverRef = useRef<HTMLDivElement>(null);
  const anchorRectRef = useRef<DOMRect | null>(null);
  const hoverTimerRef = useRef<number | null>(null);
  const selectionStartRef = useRef(0);
  const selectionEndRef = useRef(0);
  const debounceRef = useRef<number | null>(null);
  const checkSeqRef = useRef(0);
  const autoSaveRef = useRef<number | null>(null);

  // Library store — filter pills drive this
  const { prompts, folders, tags, search, selectedFolderId, selectedTag, sort, setSearch, setSelectedFolderId, setSelectedTag, refresh, refreshFolders, refreshTags } =
    usePromptLibraryStore();
  const [localSearch, setLocalSearch] = useState(search);

  useEffect(() => setLocalSearch(search), [search]);
  useEffect(() => {
    const id = window.setTimeout(() => {
      if (localSearch !== search) setSearch(localSearch);
    }, SEARCH_DEBOUNCE_MS);
    return () => window.clearTimeout(id);
  }, [localSearch, search, setSearch]);

  useEffect(() => {
    refresh();
    refreshFolders();
    refreshTags();
  }, [refresh, refreshFolders, refreshTags]);
  useEffect(() => {
    refresh();
  }, [search, selectedFolderId, selectedTag, sort, refresh]);

  // Restore draft
  useEffect(() => {
    if (!editor) return;
    const draftHtml = useWorkbenchDraftStore.getState().draftHtml;
    if (draftHtml) {
      editor.commands.setContent(draftHtml);
      setDocText(editor.state.doc.textContent);
    }
  }, [editor]);
  useEffect(() => {
    const draft = usePromptDraftStore.getState().consumeDraft();
    if (draft && editor) {
      editor.commands.setContent(draft);
      setDocText(editor.state.doc.textContent);
      useWorkbenchDraftStore.getState().setDraftHtml(editor.getHTML());
      editor.commands.focus("end");
    }
  }, [editor]);

  // History strip — last entries, live via events
  useEffect(() => {
    commands
      .listPromptHistory(3)
      .then((res) => {
        if (res.status === "ok") setHistoryEntries(res.data.slice(0, 3));
      })
      .catch(() => {});
    const unlisten = events.promptHistoryUpdatePayload.listen((event) => {
      const p = event.payload as { action: string; entry?: PromptHistoryEntry; id?: number };
      if (p.action === "added" && p.entry) setHistoryEntries((prev) => [p.entry as PromptHistoryEntry, ...prev].slice(0, 3));
      else if (p.action === "deleted" && p.id) setHistoryEntries((prev) => prev.filter((e) => e.id !== p.id));
      else if (p.action === "cleared") setHistoryEntries([]);
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  // Auto-save composer
  useEffect(() => {
    if (autoSaveRef.current) { window.clearTimeout(autoSaveRef.current); autoSaveRef.current = null; }
    if (!docText.trim()) return;
    autoSaveRef.current = window.setTimeout(() => {
      autoSaveRef.current = null;
      commands.savePromptHistoryEntry(docText).catch(() => {});
    }, PROMPT_AUTO_SAVE_DEBOUNCE_MS);
    return () => { if (autoSaveRef.current) window.clearTimeout(autoSaveRef.current); };
  }, [docText]);

  useEffect(() => {
    if (phase !== "recording") return;
    const id = setInterval(() => setElapsed((s) => s + 1), 1000);
    return () => clearInterval(id);
  }, [phase]);
  useEffect(() => {
    const p = phase;
    return () => { if (p !== "idle") commands.cancelDictation().catch(() => {}); };
  }, [phase]);

  useEffect(() => {
    if (debounceRef.current) window.clearTimeout(debounceRef.current);
    const push = () => { if (editor) editor.view.dispatch(editor.state.tr.setMeta(SPELL_CHECK_META, true)); };
    if (!docText || !spellEnabled) {
      checkSeqRef.current += 1;
      spellIssueCache.issues = [];
      push();
      return;
    }
    const seq = ++checkSeqRef.current;
    debounceRef.current = window.setTimeout(() => {
      commands.checkSpelling(docText).then((res) => {
        if (checkSeqRef.current !== seq) return;
        if (res.status === "ok") {
          spellIssueCache.issues = res.data.filter((i) => i.start < docText.length);
          push();
        }
      }).catch(() => {});
    }, SPELL_CHECK_DEBOUNCE_MS);
    return () => { if (debounceRef.current) window.clearTimeout(debounceRef.current); };
  }, [docText, spellEnabled, editor]);

  useEffect(() => {
    const hide = () => { if (hoverTimerRef.current) window.clearTimeout(hoverTimerRef.current); setActiveIssue(null); setPopoverPos(null); };
    window.addEventListener("scroll", hide, true);
    return () => window.removeEventListener("scroll", hide, true);
  }, []);
  useEffect(() => {
    if (hoverTimerRef.current) window.clearTimeout(hoverTimerRef.current);
    setActiveIssue(null); setPopoverPos(null);
  }, [docText]);
  useEffect(() => () => { if (hoverTimerRef.current) window.clearTimeout(hoverTimerRef.current); }, []);

  const issueAtPoint = (x: number, y: number): SpellingIssue | null => {
    const dom = editor?.view.dom;
    if (!dom) return null;
    for (const span of dom.querySelectorAll<HTMLElement>("[data-issue]")) {
      const r = span.getBoundingClientRect();
      if (x >= r.left && x <= r.right && y >= r.top && y <= r.bottom) {
        return spellIssueCache.issues.find((i) => i.start === Number(span.dataset.start) && i.end === Number(span.dataset.end)) ?? null;
      }
    }
    return null;
  };
  const hidePopover = () => { if (hoverTimerRef.current) window.clearTimeout(hoverTimerRef.current); setActiveIssue(null); setPopoverPos(null); };
  const showPopoverFor = (issue: SpellingIssue) => {
    const dom = editor?.view.dom; if (!dom) return;
    const span = dom.querySelector<HTMLElement>(`[data-start="${issue.start}"][data-end="${issue.end}"]`);
    if (!span) return;
    anchorRectRef.current = span.getBoundingClientRect();
    setPopoverPos({ left: anchorRectRef.current.left, top: anchorRectRef.current.bottom + 6, above: false });
    setActiveIssue(issue);
  };
  const handleEditorMouseMove = (e: React.MouseEvent<HTMLDivElement>) => {
    const pop = popoverRef.current;
    if (pop) { const r = pop.getBoundingClientRect(); if (e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom) return; }
    if (hoverTimerRef.current) window.clearTimeout(hoverTimerRef.current);
    const issue = issueAtPoint(e.clientX, e.clientY);
    if (!issue) { setActiveIssue(null); setPopoverPos(null); return; }
    hoverTimerRef.current = window.setTimeout(() => showPopoverFor(issue), SPELL_HOVER_DELAY_MS) as unknown as number;
  };
  const handleEditorMouseUp = (e: React.MouseEvent<HTMLDivElement>) => {
    if (popoverRef.current?.contains(e.target as Node)) return;
    const issue = issueAtPoint(e.clientX, e.clientY);
    if (issue) showPopoverFor(issue); else hidePopover();
  };
  useLayoutEffect(() => {
    const pop = popoverRef.current; const anchor = anchorRectRef.current;
    if (!pop || !activeIssue || !popoverPos || !anchor) return;
    const r = pop.getBoundingClientRect();
    if (!popoverPos.above && r.bottom > window.innerHeight - 8) setPopoverPos({ left: popoverPos.left, top: anchor.top - r.height - 10, above: true });
    else if (r.right > window.innerWidth - 8) setPopoverPos({ ...popoverPos, left: Math.max(8, window.innerWidth - r.width - 8) });
  }, [activeIssue, popoverPos]);

  const insertText = (t: string) => {
    editor?.chain().focus().setTextSelection({ from: selectionStartRef.current, to: selectionEndRef.current }).insertContent(t).run();
  };
  const handleMicClick = async () => {
    if (phase === "recording") {
      setPhase("transcribing");
      const res = await commands.stopDictation();
      if (res.status === "ok") insertText(res.data);
      else setError(res.error === "cancelled" || res.error === "empty" ? t("home.noSpeech") : t("home.micError", { error: res.error }));
      setPhase("idle"); return;
    }
    const { from, to } = editor?.state.selection ?? { from: 0, to: 0 };
    selectionStartRef.current = from; selectionEndRef.current = to;
    setError(null);
    const res = await commands.startDictation();
    if (res.status === "error") { setError(t("home.micError", { error: res.error })); return; }
    setElapsed(0); setPhase("recording");
  };
  const handleCancel = async () => { try { await commands.cancelDictation(); } catch {} setPhase("idle"); };
  const handleCopy = async () => { try { await navigator.clipboard.writeText(docText); toast.success(t("promptStudio.copied")); } catch {} };
  const handleClear = () => {
    editor?.chain().focus().clearContent().run();
    setDocText("");
    useWorkbenchDraftStore.getState().setDraftHtml("");
    spellIssueCache.issues = [];
    if (editor) editor.view.dispatch(editor.state.tr.setMeta(SPELL_CHECK_META, true));
  };
  const handleSend = async () => {
    if (!docText.trim()) return;
    setPasting(true);
    try {
      const res = await commands.pastePrompt(docText);
      if (res.status === "ok") toast.success(t("promptStudio.sent"));
      else toast.error(t("promptStudio.sendFailed"), { description: res.error });
    } catch (e) { toast.error(t("promptStudio.sendFailed")); }
    finally { setPasting(false); }
  };
  const applySuggestion = (issue: SpellingIssue, suggestion: string) => {
    if (!editor) return;
    const text = editor.state.doc.textContent;
    const positions = positionsFromOffsets(editor.state.doc, text);
    const from = positions?.[issue.start]; const to = positions?.[issue.end];
    if (from === undefined || to === undefined) return;
    editor.chain().focus().insertContentAt({ from, to }, suggestion).run();
    hidePopover();
  };
  const handleReuseHistory = (entry: PromptHistoryEntry) => {
    editor?.commands.setContent(entry.prompt_text);
    setDocText(entry.prompt_text);
    useWorkbenchDraftStore.getState().setDraftHtml(editor?.getHTML() ?? entry.prompt_text);
    editor?.commands.focus("end");
  };

  const visiblePrompts = useMemo(() => prompts.slice(0, libraryExpanded ? prompts.length : gridLimit), [prompts, libraryExpanded, gridLimit]);

  return (
    <div className="w-full max-w-5xl mx-auto space-y-3">
      {/* Composer card — Affinity Pixel studio: toolbar inside canvas, rounded-xl, border */}
      <div
        onMouseMove={handleEditorMouseMove}
        onMouseLeave={hidePopover}
        onMouseUp={handleEditorMouseUp}
        className="rounded-xl bg-background border border-mid-gray/20 overflow-hidden flex flex-col shadow-sm"
      >
        <div className="flex items-center justify-between border-b border-mid-gray/10 bg-mid-gray/[0.04]">
          <PromptFormatToolbar editor={editor} />
          <label className="hidden sm:flex items-center gap-1.5 text-xs text-text/50 cursor-pointer select-none shrink-0 ml-2">
            <input type="checkbox" className="sr-only peer" checked={spellEnabled} onChange={(e) => updateSetting("spell_check_enabled", e.target.checked)} />
            <span className="relative w-7 h-4 bg-mid-gray/20 rounded-full transition-colors peer-checked:bg-background-ui after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:rounded-full after:h-3 after:w-3 after:transition-all peer-checked:after:translate-x-3" />
            {t("promptStudio.spellCheck")}
          </label>
        </div>
        <div className="min-h-[160px]">
          <EditorContent editor={editor} />
        </div>
      </div>

      {activeIssue && popoverPos && (
        <div ref={popoverRef} className="fixed z-50 w-60 rounded-lg border border-mid-gray/80 bg-background p-2 space-y-1.5 shadow-lg" style={{ left: popoverPos.left, top: popoverPos.top }}>
          <p className="text-xs text-text/70">{activeIssue.message}</p>
          <div className="flex flex-wrap gap-1">
            {activeIssue.suggestions.slice(0, 3).map((s) => (
              <button key={s} onClick={() => applySuggestion(activeIssue, s)} className="px-2 py-0.5 rounded border border-logo-primary/40 bg-logo-primary/10 hover:bg-logo-primary/20 text-logo-primary font-medium text-xs">
                {s}
              </button>
            ))}
          </div>
        </div>
      )}

      {error && <p className="text-sm text-red-400">{error}</p>}

      {/* Composer Send bar — primary Send (paste to app) + Copy + Clear + Mic */}
      <div className="rounded-xl border border-mid-gray/15 bg-mid-gray/[0.04] p-2 flex items-center gap-2 flex-wrap">
        <Button
          variant={phase === "recording" ? "danger" : "primary"}
          size="sm"
          onClick={handleMicClick}
          disabled={phase === "transcribing"}
          className={phase === "recording" ? "animate-pulse" : ""}
          title={phase === "recording" ? t("home.stopRecording") : t("home.startRecording")}
          aria-label={phase === "recording" ? t("home.stopRecording") : t("home.startRecording")}
        >
          {phase === "transcribing" ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Mic className="w-3.5 h-3.5" />}
        </Button>
        {phase === "recording" && (
          <span className="flex items-center gap-1.5 text-sm text-red-400">
            <span className="w-2 h-2 rounded-full bg-red-500 animate-pulse" />
            {t("home.recording")} {formatElapsed(elapsed)}
          </span>
        )}
        {phase === "transcribing" && (
          <span className="flex items-center gap-2 text-sm text-text/60">
            <Loader2 className="w-3.5 h-3.5 animate-spin" />
            {t("home.transcribing")}
            <Button variant="ghost" size="sm" onClick={handleCancel}>{t("home.cancel")}</Button>
          </span>
        )}
        <div className="flex-1 min-w-0" />
        <Button variant="secondary" size="sm" onClick={handleCopy} disabled={!docText}>
          <Clipboard className="w-3.5 h-3.5" /> {t("promptStudio.copy")}
        </Button>
        <Button variant="ghost" size="sm" onClick={handleClear} disabled={!docText}>
          <Eraser className="w-3.5 h-3.5" /> {t("promptStudio.clear")}
        </Button>
        <Button variant="primary" size="sm" onClick={handleSend} disabled={!docText.trim() || pasting} className="min-w-[88px] justify-center">
          {pasting ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Send className="w-3.5 h-3.5" />}
          {pasting ? t("promptStudio.sending") : t("promptStudio.send")}
        </Button>
      </div>

      {/* Library canvas — search + folder/tag pills + compact grid */}
      <div className="rounded-xl border border-mid-gray/15 bg-background p-3 space-y-3">
        <div className="flex items-center justify-between gap-2">
          <p className="text-xs font-semibold tracking-widest text-text/40 uppercase">{t("promptStudio.library")}</p>
          <Button variant="ghost" size="sm" onClick={() => { setEditing(null); setEditorOpen(true); }} className="h-7 text-xs">
            + {t("promptStudio.newPrompt")}
          </Button>
        </div>

        <div className="flex gap-2 items-center flex-wrap">
          <div className="relative flex-1 min-w-[160px]">
            <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-text/30 pointer-events-none" />
            <input
              value={localSearch}
              onChange={(e) => setLocalSearch(e.target.value)}
              placeholder={t("promptStudio.searchPlaceholder")}
              aria-label={t("promptStudio.searchAriaLabel")}
              className="w-full pl-8 pr-3 py-1.5 text-sm bg-mid-gray/10 border border-mid-gray/20 rounded-full focus:outline-none focus:ring-1 focus:ring-logo-primary placeholder:text-text/30"
            />
          </div>
          {/* Folder pills */}
          <div className="flex items-center gap-1.5 overflow-x-auto scrollbar-none flex-nowrap">
            <button
              onClick={() => setSelectedFolderId(null)}
              className={`shrink-0 px-3 py-1 rounded-full text-xs font-medium border transition-colors ${selectedFolderId === null ? "bg-logo-primary text-white border-logo-primary" : "bg-mid-gray/10 border-mid-gray/20 text-text/60 hover:bg-mid-gray/15"}`}
            >
              {t("promptStudio.allFolders")}
            </button>
            {folders.slice(0, 8).map((f) => (
              <button
                key={f.id}
                onClick={() => setSelectedFolderId(f.id)}
                className={`shrink-0 px-3 py-1 rounded-full text-xs font-medium border transition-colors ${selectedFolderId === f.id ? "bg-logo-primary text-white border-logo-primary" : "bg-mid-gray/10 border-mid-gray/20 text-text/60 hover:bg-mid-gray/15"}`}
              >
                {f.name}
              </button>
            ))}
          </div>
        </div>

        {tags.length > 0 && (
          <div className="flex items-center gap-1.5 flex-wrap">
            <button
              onClick={() => setSelectedTag(null)}
              className={`px-2.5 py-1 rounded-full text-[11px] font-medium border ${selectedTag === null ? "bg-text text-background border-text" : "bg-mid-gray/10 border-mid-gray/20 text-text/50 hover:bg-mid-gray/15"}`}
            >
              {t("promptStudio.allTags")}
            </button>
            {tags.slice(0, 10).map((tag) => (
              <button
                key={tag.id}
                onClick={() => setSelectedTag(tag.name)}
                className={`px-2.5 py-1 rounded-full text-[11px] font-medium border ${selectedTag === tag.name ? "bg-text text-background border-text" : "bg-mid-gray/10 border-mid-gray/20 text-text/50 hover:bg-mid-gray/15"}`}
              >
                #{tag.name}
              </button>
            ))}
          </div>
        )}

        {visiblePrompts.length === 0 ? (
          <p className="text-sm text-text/40 text-center py-6">{search || selectedFolderId || selectedTag ? t("promptStudio.noResults") : t("promptStudio.noPrompts")}</p>
        ) : (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
            {visiblePrompts.map((p) => (
              <StudioPromptCard
                key={p.id}
                prompt={p}
                onEdit={(pr) => { setEditing(pr); setEditorOpen(true); }}
                onRefresh={refresh}
                onInsertToComposer={(text) => {
                  editor?.chain().focus().insertContent(text + " ").run();
                  setDocText(editor?.state.doc.textContent ?? "");
                }}
              />
            ))}
          </div>
        )}

        <div className="flex items-center justify-center gap-2 pt-1">
          {!libraryExpanded && prompts.length > gridLimit && (
            <button onClick={() => setGridLimit((n) => Math.min(n + 6, prompts.length))} className="text-xs text-logo-primary hover:underline">
              {t("promptStudio.showAll")} ({prompts.length - gridLimit} {t("common.open").toLowerCase()})
            </button>
          )}
          {gridLimit > 6 && !libraryExpanded && (
            <button onClick={() => setGridLimit(6)} className="text-xs text-text/40 hover:text-text">
              {t("promptStudio.showLess")}
            </button>
          )}
          <span className="text-text/20 text-xs">·</span>
          <button onClick={() => setLibraryExpanded((v) => !v)} className="text-xs text-text/60 hover:text-text inline-flex items-center gap-1">
            {libraryExpanded ? (
              <>
                <ChevronUp className="w-3 h-3" /> {t("promptStudio.showLess")}
              </>
            ) : (
              <>
                <ExternalLink className="w-3 h-3" /> {t("promptStudio.showAll")}
              </>
            )}
          </button>
        </div>

        {libraryExpanded && (
          <div className="border-t border-mid-gray/15 pt-4 mt-2">
            <PromptLibraryView />
          </div>
        )}
      </div>

      {/* History — thin collapsible strip, last 3 inserts */}
      <div className="rounded-xl border border-mid-gray/15 bg-mid-gray/[0.03] overflow-hidden">
        <button
          onClick={() => setHistoryCollapsed((v) => !v)}
          className="w-full flex items-center justify-between px-3 py-2 hover:bg-mid-gray/10 transition-colors"
        >
          <span className="text-xs font-semibold tracking-widest text-text/40 uppercase">{t("promptStudio.history")}</span>
          <span className="flex items-center gap-2 text-xs text-text/40">
            {historyEntries.length > 0 && <span>{historyEntries.length}</span>}
            {historyCollapsed ? <ChevronDown className="w-3.5 h-3.5" /> : <ChevronUp className="w-3.5 h-3.5" />}
          </span>
        </button>
        {!historyCollapsed && (
          <div className="px-3 pb-3 space-y-2">
            {historyEntries.length === 0 ? (
              <p className="text-xs text-text/40 py-2">{t("promptStudio.historyEmpty")}</p>
            ) : (
              historyEntries.map((e) => (
                <div key={e.id} className="flex items-start justify-between gap-3 p-2 rounded-lg border border-mid-gray/10 bg-background">
                  <div className="min-w-0 flex-1">
                    <p className="text-xs text-text/50">{formatDateTime(String(e.timestamp), i18n.language)}</p>
                    <p className="text-sm whitespace-pre-wrap break-words line-clamp-2">{e.prompt_text}</p>
                  </div>
                  <div className="flex items-center gap-1 shrink-0">
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={async () => { await navigator.clipboard.writeText(e.prompt_text); toast.success(t("promptStudio.copied")); }}
                      title={t("common.copy")}
                      className="px-2"
                    >
                      <Copy className="w-3 h-3" />
                    </Button>
                    <Button variant="secondary" size="sm" onClick={() => handleReuseHistory(e)}>{t("promptStudio.historyReuse")}</Button>
                  </div>
                </div>
              ))
            )}
          </div>
        )}
      </div>

      <PromptEditor open={editorOpen} onOpenChange={setEditorOpen} prompt={editing} folders={folders} onSaved={refresh} />
    </div>
  );
};
