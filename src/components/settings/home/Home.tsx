import React, { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import Underline from "@tiptap/extension-underline";
import Placeholder from "@tiptap/extension-placeholder";
import {
  Bold,
  Clipboard,
  Code2,
  Eraser,
  ExternalLink,
  Heading1,
  Heading2,
  Heading3,
  Italic,
  List,
  ListOrdered,
  Loader2,
  Mic,
  NotebookPen,
  Pilcrow,
  Quote,
  Redo2,
  Send,
  Strikethrough,
  Underline as UnderlineIcon,
  Undo2,
} from "lucide-react";
import { toast } from "sonner";
import { openUrl } from "@tauri-apps/plugin-opener";
import { commands } from "@/bindings";
import type { SpellingIssue } from "@/bindings";
import { useModelStore } from "@/stores/modelStore";
import { usePromptDraftStore } from "@/stores/promptDraftStore";
import { useSettings } from "@/hooks/useSettings";
import { Button } from "@/components/ui/Button";
import type { SettingsSectionProps } from "@/lib/types/navigation";
import {
  positionsFromOffsets,
  spellIssueCache,
  SpellCheckExtension,
  SPELL_CHECK_META,
} from "./spellCheckExtension";
import { SPELL_CHECK_DEBOUNCE_MS } from "@/lib/constants/debounce";

const SPELL_HOVER_DELAY_MS = 250;
const PROMPT_AUTO_SAVE_DEBOUNCE_MS = 1500;

// Apple guide for the macOS permission the OS speech model needs.
const SPEECH_RECOGNITION_HELP_URL =
  "https://support.apple.com/guide/mac-help/control-access-to-speech-recognition-on-mac-mchl48fbbd25/mac";

type DictationPhase = "idle" | "recording" | "transcribing";

const formatElapsed = (seconds: number): string => {
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}:${String(secs).padStart(2, "0")}`;
};

const ToolbarButton: React.FC<{
  onClick: () => void;
  title: string;
  active?: boolean;
  disabled?: boolean;
  children: React.ReactNode;
}> = ({ onClick, title, active, disabled, children }) => (
  <button
    type="button"
    onClick={onClick}
    title={title}
    disabled={disabled}
    className={`p-1.5 rounded-md transition-colors cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed ${
      active
        ? "bg-logo-primary/20 text-logo-primary"
        : "text-text/60 hover:bg-mid-gray/10 hover:text-text"
    }`}
  >
    {children}
  </button>
);

export const Home: React.FC<SettingsSectionProps> = ({ onNavigate }) => {
  const { t } = useTranslation();
  const { settings, updateSetting } = useSettings();
  const { currentModel, models, selectModel } = useModelStore();

  const [phase, setPhase] = useState<DictationPhase>("idle");
  const [error, setError] = useState<string | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const [pasting, setPasting] = useState(false);
  const [osSpeechAvailable, setOsSpeechAvailable] = useState(false);
  const [grantingPermission, setGrantingPermission] = useState(false);

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
      Underline,
      Placeholder.configure({ placeholder: t("home.placeholder") }),
      SpellCheckExtension,
    ],
    content: "",
    editorProps: {
      attributes: {
        class: "ptap-body",
        spellcheck: "false",
      },
    },
    onUpdate: ({ editor }) => {
      setDocText(editor.state.doc.textContent);
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

  const hasUsableModel =
    !!currentModel && models.some((model) => model.is_downloaded);

  // Pick up a prompt handed over from the history page ("reuse").
  useEffect(() => {
    const draft = usePromptDraftStore.getState().consumeDraft();
    if (draft && editor) {
      editor.commands.setContent(draft);
      setDocText(draft);
      editor.commands.focus("end");
    }
  }, [editor]);

  // Auto-save written prompts to history. Runs only while the box holds
  // non-empty text and the user has stopped typing for the debounce window;
  // the backend dedupes against the most recent entry, so editing a prompt
  // bumps its entry instead of creating copies. Dictation inserts go through
  // the same text change and are saved too.
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

  // Check whether the OS speech engine is available for the no-model hint
  useEffect(() => {
    let cancelled = false;
    commands
      .osSpeechAvailable()
      .then((available) => {
        if (!cancelled) setOsSpeechAvailable(available);
      })
      .catch((err) => {
        console.error("Failed to check OS speech availability:", err);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Cancel any in-flight dictation when the component unmounts
  useEffect(() => {
    const phaseAtUnmount = phase;
    return () => {
      if (phaseAtUnmount !== "idle") {
        commands.cancelDictation().catch(() => {});
      }
    };
  }, [phase]);

  // Debounced spell check of the editor text. Runs only when spell checking is
  // enabled; a stale-response guard drops results that outlive their text.
  // Fresh issues are cached for the decoration extension and the popover, then
  // pushed into the editor with a meta transaction that rebuilds the
  // wavy-underline decorations.
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

  // Hide the suggestion popover on any scroll (editor or page), since the
  // word it anchors to moves with the text.
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

  // The text changed — the underline positions moved, so drop the popover.
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

  // Issue whose underlined span's (viewport-space) rect contains the point.
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

  // Word-like hover behavior: a short delay before opening so the popover
  // doesn't chase the cursor across underlined text mid-selection.
  const handlePromptMouseMove = (event: React.MouseEvent<HTMLDivElement>) => {
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
    hoverTimerRef.current = window.setTimeout(() => {
      hoverTimerRef.current = null;
      showPopoverFor(issue);
    }, SPELL_HOVER_DELAY_MS);
  };

  // Clicking an underlined word opens its suggestions (like Word); clicking
  // anywhere else in the box closes the popover.
  const handleEditorMouseUp = (event: React.MouseEvent<HTMLDivElement>) => {
    // A release inside the popover must not dismiss it before the click.
    if (popoverRef.current?.contains(event.target as Node)) return;
    const issue = issueAtPoint(event.clientX, event.clientY);
    if (issue) {
      showPopoverFor(issue);
    } else {
      hidePopover();
    }
  };

  // Keep the popover on-screen: flip above the word when it would overflow
  // the bottom edge, clamp to the right edge.
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
    setIssuesCleared();
  };

  const setIssuesCleared = () => {
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

  const handleUseOnDevice = async () => {
    setGrantingPermission(true);
    try {
      await commands.osSpeechRequestAuthorization();
      await selectModel("os-speech");
    } catch (err) {
      console.error("Failed to request OS speech authorization:", err);
    } finally {
      setGrantingPermission(false);
    }
  };

  const tb = (titleKey: string) => t(`home.toolbar.${titleKey}`);

  // Re-render hook for the format toolbar: transactions (including pure
  // selection moves) bump `editorVersion`, which refreshes active states.
  void editorVersion;

  const toolbar = editor ? (
    <div className="flex items-center gap-0.5 flex-wrap px-2 py-1 border-b border-mid-gray/20">
      <ToolbarButton
        title={tb("undo")}
        disabled={!editor.can().undo()}
        onClick={() => editor.chain().focus().undo().run()}
      >
        <Undo2 className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title={tb("redo")}
        disabled={!editor.can().redo()}
        onClick={() => editor.chain().focus().redo().run()}
      >
        <Redo2 className="w-3.5 h-3.5" />
      </ToolbarButton>
      <div className="w-px h-4 mx-1 bg-mid-gray/30" />
      <ToolbarButton
        title={tb("paragraph")}
        active={editor.isActive("paragraph")}
        onClick={() => editor.chain().focus().setParagraph().run()}
      >
        <Pilcrow className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title={tb("heading1")}
        active={editor.isActive("heading", { level: 1 })}
        onClick={() => editor.chain().focus().toggleHeading({ level: 1 }).run()}
      >
        <Heading1 className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title={tb("heading2")}
        active={editor.isActive("heading", { level: 2 })}
        onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()}
      >
        <Heading2 className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title={tb("heading3")}
        active={editor.isActive("heading", { level: 3 })}
        onClick={() => editor.chain().focus().toggleHeading({ level: 3 }).run()}
      >
        <Heading3 className="w-3.5 h-3.5" />
      </ToolbarButton>
      <div className="w-px h-4 mx-1 bg-mid-gray/30" />
      <ToolbarButton
        title={tb("bold")}
        active={editor.isActive("bold")}
        onClick={() => editor.chain().focus().toggleBold().run()}
      >
        <Bold className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title={tb("italic")}
        active={editor.isActive("italic")}
        onClick={() => editor.chain().focus().toggleItalic().run()}
      >
        <Italic className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title={tb("underline")}
        active={editor.isActive("underline")}
        onClick={() => editor.chain().focus().toggleUnderline().run()}
      >
        <UnderlineIcon className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title={tb("strike")}
        active={editor.isActive("strike")}
        onClick={() => editor.chain().focus().toggleStrike().run()}
      >
        <Strikethrough className="w-3.5 h-3.5" />
      </ToolbarButton>
      <div className="w-px h-4 mx-1 bg-mid-gray/30" />
      <ToolbarButton
        title={tb("bulletList")}
        active={editor.isActive("bulletList")}
        onClick={() => editor.chain().focus().toggleBulletList().run()}
      >
        <List className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title={tb("orderedList")}
        active={editor.isActive("orderedList")}
        onClick={() => editor.chain().focus().toggleOrderedList().run()}
      >
        <ListOrdered className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title={tb("blockquote")}
        active={editor.isActive("blockquote")}
        onClick={() => editor.chain().focus().toggleBlockquote().run()}
      >
        <Quote className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title={tb("codeBlock")}
        active={editor.isActive("codeBlock")}
        onClick={() => editor.chain().focus().toggleCodeBlock().run()}
      >
        <Code2 className="w-3.5 h-3.5" />
      </ToolbarButton>
    </div>
  ) : null;

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <h1 className="text-xl font-semibold mb-2">{t("home.title")}</h1>

      {/* No usable model hint — the OS speech engine needs no download */}
      {!hasUsableModel && (
        <div className="flex items-center justify-between gap-3 p-3 rounded-lg border border-warning/40 bg-warning/10">
          <div className="space-y-1">
            <p className="text-sm font-medium text-text/80">
              {t("home.noModel")}
            </p>
            <p className="text-sm text-text/60">
              {t("home.noModelDescription")}
            </p>
          </div>
          <div className="flex flex-col items-end gap-1.5 shrink-0">
            {osSpeechAvailable && (
              <Button
                variant="secondary"
                size="sm"
                disabled={grantingPermission}
                onClick={handleUseOnDevice}
                className="shrink-0"
              >
                {grantingPermission ? (
                  <Loader2 className="w-3.5 h-3.5 animate-spin" />
                ) : (
                  t("home.useOnDevice")
                )}
              </Button>
            )}
            <button
              type="button"
              onClick={() => openUrl(SPEECH_RECOGNITION_HELP_URL)}
              className="flex items-center gap-1 text-xs text-logo-primary hover:underline"
            >
              <ExternalLink className="w-3 h-3" />
              {t("home.grantAccessHelp")}
            </button>
          </div>
        </div>
      )}

      {/* Top row: spell-check toggle + link to the pasted-prompts page */}
      <div className="flex items-center justify-between gap-2">
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
        <button
          type="button"
          onClick={() => onNavigate?.("prompt-history")}
          title={t("home.history.open")}
          className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-sm font-medium text-text/60 hover:bg-mid-gray/10 hover:text-text transition-colors"
        >
          <NotebookPen className="w-4 h-4" />
          {t("home.history.title")}
        </button>
      </div>

      {/* Prompt editor with format toolbar and Harper underline decorations */}
      <div
        onMouseMove={handlePromptMouseMove}
        onMouseLeave={hidePopover}
        onMouseUp={handleEditorMouseUp}
        className="relative rounded-md bg-mid-gray/10 border border-mid-gray/80 transition-[background-color,border-color] duration-150 hover:bg-logo-primary/10 hover:border-logo-primary focus-within:bg-logo-primary/10 focus-within:border-logo-primary"
      >
        {toolbar}
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

      {/* Toolbar */}
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
