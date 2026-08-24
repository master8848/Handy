import React, { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import Placeholder from "@tiptap/extension-placeholder";
import {
  Clipboard,
  Eraser,
  FileText,
  Loader2,
  Mic,
  Send,
} from "lucide-react";
import { toast } from "sonner";
import { commands } from "@/bindings";
import type { SpellingIssue } from "@/bindings";
import { useSettings } from "@/hooks/useSettings";
import { Button } from "@/components/ui/Button";
import { PromptFormatToolbar } from "@/components/shared";
import { useWorkbenchDraftStore } from "@/stores/workbenchDraftStore";
import { usePromptDraftStore } from "@/stores/promptDraftStore";
import {
  positionsFromOffsets,
  spellIssueCache,
  SpellCheckExtension,
  SPELL_CHECK_META,
} from "@/components/settings/home/spellCheckExtension";
import { SPELL_CHECK_DEBOUNCE_MS } from "@/lib/constants/debounce";

const SPELL_HOVER_DELAY_MS = 250;
const PROMPT_AUTO_SAVE_DEBOUNCE_MS = 1500;

type DictationPhase = "idle" | "recording" | "transcribing";

const formatElapsed = (seconds: number): string => {
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}:${String(secs).padStart(2, "0")}`;
};

/**
 * Prompt Workbench: the "Prompt" tab of the main window. A large full-width
 * tiptap canvas (styled after the Transcribe tab) with the shared format
 * toolbar on top and mic / Copy / Clear / Paste actions below. The draft is
 * kept in `useWorkbenchDraftStore` so it survives tab switches, and non-empty
 * text auto-saves to the pasted-prompts history like the dictation box does.
 */
export const PromptWorkbench: React.FC = () => {
  const { t } = useTranslation();
  const { settings, updateSetting } = useSettings();

  const [phase, setPhase] = useState<DictationPhase>("idle");
  const [error, setError] = useState<string | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const [pasting, setPasting] = useState(false);

  // Plain text of the editor (raw doc textContent), driving the debounced
  // spell check, auto-save, and button disable states.
  const [docText, setDocText] = useState("");
  const [activeIssue, setActiveIssue] = useState<SpellingIssue | null>(null);
  const [popoverPos, setPopoverPos] = useState<{
    left: number;
    top: number;
    above: boolean;
  } | null>(null);

  const editor = useEditor({
    extensions: [
      StarterKit,
      Placeholder.configure({ placeholder: t("workbench.placeholder") }),
      SpellCheckExtension,
    ],
    content: "",
    editorProps: {
      attributes: {
        class: "ptap-body ptap-workbench",
        spellcheck: "false",
      },
    },
    onUpdate: ({ editor }) => {
      const text = editor.state.doc.textContent;
      setDocText(text);
      useWorkbenchDraftStore.getState().setDraftHtml(editor.getHTML());
    },
    onTransaction: ({ editor }) => {
      setEditorVersion((version) => version + 1);
    },
  });

  // Bumped on every editor transaction so the format toolbar re-reads its
  // active/disabled states (selection moves don't change docText).
  const [editorVersion, setEditorVersion] = useState(0);

  const popoverRef = useRef<HTMLDivElement>(null);
  const anchorRectRef = useRef<DOMRect | null>(null);
  const hoverTimerRef = useRef<number | null>(null);
  const selectionStartRef = useRef(0);
  const selectionEndRef = useRef(0);
  const debounceRef = useRef<number | null>(null);
  const checkSeqRef = useRef(0);
  const autoSaveRef = useRef<number | null>(null);

  // Restore the persisted draft once the editor exists.
  useEffect(() => {
    if (!editor) return;
    const draftHtml = useWorkbenchDraftStore.getState().draftHtml;
    if (draftHtml) {
      editor.commands.setContent(draftHtml);
      setDocText(editor.state.doc.textContent);
      editor.commands.focus("end");
    }
  }, [editor]);

  // Pick up a prompt handed over from the history page ("reuse").
  useEffect(() => {
    const draft = usePromptDraftStore.getState().consumeDraft();
    if (draft && editor) {
      editor.commands.setContent(draft);
      setDocText(editor.state.doc.textContent);
      useWorkbenchDraftStore.getState().setDraftHtml(editor.getHTML());
      editor.commands.focus("end");
    }
  }, [editor]);

  // Auto-save written prompts to history (same contract as the dictation box):
  // runs only while the canvas holds non-empty text and typing has settled;
  // the backend dedupes against the most recent entry.
  useEffect(() => {
    if (autoSaveRef.current) {
      window.clearTimeout(autoSaveRef.current);
      autoSaveRef.current = null;
    }
    if (!docText.trim()) return;
    autoSaveRef.current = window.setTimeout(() => {
      autoSaveRef.current = null;
      commands.savePromptHistoryEntry(docText).catch((err) => {
        console.error("Failed to auto-save prompt history entry:", err);
      });
    }, PROMPT_AUTO_SAVE_DEBOUNCE_MS);
    return () => {
      if (autoSaveRef.current) {
        window.clearTimeout(autoSaveRef.current);
        autoSaveRef.current = null;
      }
    };
  }, [docText]);

  // Elapsed timer while recording
  useEffect(() => {
    if (phase !== "recording") return;
    const interval = setInterval(
      () => setElapsed((seconds) => seconds + 1),
      1000,
    );
    return () => clearInterval(interval);
  }, [phase]);

  // Cancel any in-flight dictation when the component unmounts
  useEffect(() => {
    const phaseAtUnmount = phase;
    return () => {
      if (phaseAtUnmount !== "idle") {
        commands.cancelDictation().catch(() => {});
      }
    };
  }, [phase]);

  // Debounced spell check of the editor text (same behavior as Home).
  useEffect(() => {
    if (debounceRef.current) {
      window.clearTimeout(debounceRef.current);
      debounceRef.current = null;
    }
    const pushToEditor = () => {
      if (!editor) return;
      editor.view.dispatch(editor.state.tr.setMeta(SPELL_CHECK_META, true));
    };
    if (!docText || !(settings?.spell_check_enabled ?? true)) {
      checkSeqRef.current += 1;
      spellIssueCache.issues = [];
      pushToEditor();
      return;
    }
    const seq = ++checkSeqRef.current;
    debounceRef.current = window.setTimeout(() => {
      debounceRef.current = null;
      commands
        .checkSpelling(docText)
        .then((res) => {
          if (checkSeqRef.current !== seq) return;
          if (res.status === "ok") {
            spellIssueCache.issues = res.data.filter(
              (i) => i.start < docText.length,
            );
            pushToEditor();
          }
        })
        .catch(() => {
          // A failed check isn't worth surfacing; keep existing underlines.
        });
    }, SPELL_CHECK_DEBOUNCE_MS);
    return () => {
      if (debounceRef.current) {
        window.clearTimeout(debounceRef.current);
        debounceRef.current = null;
      }
    };
  }, [docText, settings?.spell_check_enabled, editor]);

  // Hide the suggestion popover on any scroll, since the word it anchors to
  // moves with the text.
  useEffect(() => {
    const hideOnScroll = () => {
      if (hoverTimerRef.current) {
        window.clearTimeout(hoverTimerRef.current);
        hoverTimerRef.current = null;
      }
      setActiveIssue(null);
      setPopoverPos(null);
    };
    window.addEventListener("scroll", hideOnScroll, true);
    return () => window.removeEventListener("scroll", hideOnScroll, true);
  }, []);

  // The text changed — underline positions moved — so drop the popover.
  useEffect(() => {
    if (hoverTimerRef.current) {
      window.clearTimeout(hoverTimerRef.current);
      hoverTimerRef.current = null;
    }
    setActiveIssue(null);
    setPopoverPos(null);
  }, [docText]);

  // Clear the pending hover timer on unmount.
  useEffect(() => {
    return () => {
      if (hoverTimerRef.current) {
        window.clearTimeout(hoverTimerRef.current);
      }
    };
  }, []);

  const issueAtPoint = (
    clientX: number,
    clientY: number,
  ): SpellingIssue | null => {
    const dom = editor?.view.dom;
    if (!dom) return null;
    for (const span of dom.querySelectorAll<HTMLElement>("[data-issue]")) {
      const rect = span.getBoundingClientRect();
      if (
        clientX >= rect.left &&
        clientX <= rect.right &&
        clientY >= rect.top &&
        clientY <= rect.bottom
      ) {
        return (
          spellIssueCache.issues.find(
            (issue) =>
              issue.start === Number(span.dataset.start) &&
              issue.end === Number(span.dataset.end),
          ) ?? null
        );
      }
    }
    return null;
  };

  const hidePopover = () => {
    if (hoverTimerRef.current) {
      window.clearTimeout(hoverTimerRef.current);
      hoverTimerRef.current = null;
    }
    setActiveIssue(null);
    setPopoverPos(null);
  };

  const showPopoverFor = (issue: SpellingIssue) => {
    const dom = editor?.view.dom;
    if (!dom) return;
    const span = dom.querySelector<HTMLElement>(
      `[data-start="${issue.start}"][data-end="${issue.end}"]`,
    );
    if (!span) return;
    anchorRectRef.current = span.getBoundingClientRect();
    setPopoverPos({
      left: anchorRectRef.current.left,
      top: anchorRectRef.current.bottom + 6,
      above: false,
    });
    setActiveIssue(issue);
  };

  const handleEditorMouseMove = (event: React.MouseEvent<HTMLDivElement>) => {
    const popover = popoverRef.current;
    if (popover) {
      const rect = popover.getBoundingClientRect();
      if (
        event.clientX >= rect.left &&
        event.clientX <= rect.right &&
        event.clientY >= rect.top &&
        event.clientY <= rect.bottom
      ) {
        return; // over the popover itself — keep it open
      }
    }
    if (hoverTimerRef.current) {
      window.clearTimeout(hoverTimerRef.current);
      hoverTimerRef.current = null;
    }
    const issue = issueAtPoint(event.clientX, event.clientY);
    if (!issue) {
      setActiveIssue(null);
      setPopoverPos(null);
      return;
    }
    hoverTimerRef.current = window.setTimeout(
      () => showPopoverFor(issue),
      SPELL_HOVER_DELAY_MS,
    );
  };

  const handleEditorMouseUp = (event: React.MouseEvent<HTMLDivElement>) => {
    if (popoverRef.current?.contains(event.target as Node)) return;
    const issue = issueAtPoint(event.clientX, event.clientY);
    if (issue) {
      showPopoverFor(issue);
    } else {
      hidePopover();
    }
  };

  // Keep the popover on-screen: flip above when it would overflow the bottom,
  // clamp to the right edge.
  useLayoutEffect(() => {
    const popover = popoverRef.current;
    const anchor = anchorRectRef.current;
    if (!popover || !activeIssue || !popoverPos || !anchor) return;
    const rect = popover.getBoundingClientRect();
    if (!popoverPos.above && rect.bottom > window.innerHeight - 8) {
      setPopoverPos({
        left: popoverPos.left,
        top: anchor.top - rect.height - 10,
        above: true,
      });
    } else if (
      rect.right > window.innerWidth - 8 &&
      popoverPos.left !== Math.max(8, window.innerWidth - rect.width - 8)
    ) {
      setPopoverPos({
        ...popoverPos,
        left: Math.max(8, window.innerWidth - rect.width - 8),
      });
    }
  }, [activeIssue, popoverPos]);

  const insertText = (transcript: string) => {
    editor
      ?.chain()
      .focus()
      .setTextSelection({
        from: selectionStartRef.current,
        to: selectionEndRef.current,
      })
      .insertContent(transcript)
      .run();
  };

  const handleMicClick = async () => {
    if (phase === "recording") {
      setPhase("transcribing");
      const result = await commands.stopDictation();
      if (result.status === "ok") {
        insertText(result.data);
      } else {
        setError(
          result.error === "cancelled" || result.error === "empty"
            ? t("home.noSpeech")
            : t("home.micError", { error: result.error }),
        );
      }
      setPhase("idle");
      return;
    }

    const { from, to } = editor?.state.selection ?? { from: 0, to: 0 };
    selectionStartRef.current = from;
    selectionEndRef.current = to;
    setError(null);
    const result = await commands.startDictation();
    if (result.status === "error") {
      setError(t("home.micError", { error: result.error }));
      return;
    }
    setElapsed(0);
    setPhase("recording");
  };

  const handleCancel = async () => {
    try {
      await commands.cancelDictation();
    } catch (err) {
      console.error("Failed to cancel dictation:", err);
    }
    setPhase("idle");
  };

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(docText);
      toast.success(t("common.copied"));
    } catch (err) {
      console.error("Failed to copy text:", err);
    }
  };

  const handleClear = () => {
    editor?.chain().focus().clearContent().run();
    setDocText("");
    useWorkbenchDraftStore.getState().setDraftHtml("");
    spellIssueCache.issues = [];
    if (editor) {
      editor.view.dispatch(editor.state.tr.setMeta(SPELL_CHECK_META, true));
    }
  };

  const handlePaste = async () => {
    if (!docText.trim()) return;
    setPasting(true);
    try {
      const result = await commands.pastePrompt(docText);
      if (result.status === "ok") {
        toast.success(t("home.pasted"));
      } else {
        toast.error(t("home.pasteFailed"), {
          description: result.error,
        });
      }
    } catch (err) {
      console.error("Failed to paste prompt:", err);
      toast.error(t("home.pasteFailed"));
    } finally {
      setPasting(false);
    }
  };

  const applySuggestion = (issue: SpellingIssue, suggestion: string) => {
    if (!editor) return;
    const text = editor.state.doc.textContent;
    const positions = positionsFromOffsets(editor.state.doc, text);
    const from = positions?.[issue.start];
    const to = positions?.[issue.end];
    if (from === undefined || to === undefined) return;
    editor.chain().focus().insertContentAt({ from, to }, suggestion).run();
    hidePopover();
  };

  const openPalette = () => {
    window.dispatchEvent(new Event("handy:open-palette"));
  };

  // Refresh hook for the format toolbar active states.
  void editorVersion;

  return (
    <div className="w-full max-w-5xl mx-auto space-y-4">
      <div className="flex items-center justify-between gap-2">
        <p className="text-sm font-medium flex items-center gap-2">
          <FileText className="w-4 h-4" />
          {t("workbench.title")}
        </p>
        <label className="flex items-center gap-2 text-sm text-text/60 cursor-pointer select-none">
          <input
            type="checkbox"
            className="sr-only peer"
            checked={settings?.spell_check_enabled ?? true}
            onChange={(e) =>
              updateSetting("spell_check_enabled", e.target.checked)
            }
          />
          <div className="relative w-9 h-5 bg-mid-gray/20 rounded-full transition-colors peer-checked:bg-background-ui after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border after:border-gray-300 after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:after:translate-x-full" />
          {t("home.spellCheck")}
        </label>
      </div>

      {/* Full-width editing canvas */}
      <div
        onMouseMove={handleEditorMouseMove}
        onMouseLeave={hidePopover}
        onMouseUp={handleEditorMouseUp}
        className="rounded-xl bg-mid-gray/10 border border-mid-gray/30 transition-colors focus-within:border-logo-primary hover:border-mid-gray/60 flex flex-col min-h-[50vh]"
      >
        <PromptFormatToolbar editor={editor} />
        <EditorContent editor={editor} />
      </div>

      {/* Hover suggestion popover, anchored to the underlined word */}
      {activeIssue && popoverPos && (
        <div
          ref={popoverRef}
          className="fixed z-50 w-60 rounded-lg border border-mid-gray/80 bg-background p-2 space-y-1.5 shadow-lg"
          style={{ left: popoverPos.left, top: popoverPos.top }}
        >
          <p className="text-xs text-text/70">{activeIssue.message}</p>
          <div className="flex flex-wrap gap-1">
            {activeIssue.suggestions.slice(0, 3).map((suggestion) => (
              <button
                key={suggestion}
                onClick={() => applySuggestion(activeIssue, suggestion)}
                className="px-2 py-0.5 rounded border border-logo-primary/40 bg-logo-primary/10 hover:bg-logo-primary/20 text-logo-primary font-medium text-xs"
              >
                {suggestion}
              </button>
            ))}
          </div>
        </div>
      )}

      {error && <p className="text-sm text-red-400">{error}</p>}

      {/* Actions row */}
      <div className="flex items-center gap-2 flex-wrap">
        <Button
          variant={phase === "recording" ? "danger" : "primary"}
          size="sm"
          onClick={handleMicClick}
          disabled={phase === "transcribing"}
          className={phase === "recording" ? "animate-pulse" : ""}
          title={
            phase === "recording"
              ? t("home.stopRecording")
              : t("home.startRecording")
          }
        >
          {phase === "transcribing" ? (
            <Loader2 className="w-3.5 h-3.5 animate-spin" />
          ) : (
            <Mic className="w-3.5 h-3.5" />
          )}
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
            <Button variant="ghost" size="sm" onClick={handleCancel}>
              {t("home.cancel")}
            </Button>
          </span>
        )}

        <div className="flex-1" />

        <Button variant="secondary" size="sm" onClick={openPalette}>
          {t("workbench.insertFromLibrary")}
        </Button>

        <Button
          variant="secondary"
          size="sm"
          onClick={handleCopy}
          disabled={!docText}
        >
          <Clipboard className="w-3.5 h-3.5" />
          {t("common.copy")}
        </Button>

        <Button
          variant="secondary"
          size="sm"
          onClick={handleClear}
          disabled={!docText}
        >
          <Eraser className="w-3.5 h-3.5" />
          {t("common.clear")}
        </Button>

        <Button
          variant="primary"
          size="sm"
          onClick={handlePaste}
          disabled={!docText.trim() || pasting}
        >
          {pasting ? (
            <Loader2 className="w-3.5 h-3.5 animate-spin" />
          ) : (
            <Send className="w-3.5 h-3.5" />
          )}
          {t("home.paste")}
        </Button>
      </div>
    </div>
  );
};
