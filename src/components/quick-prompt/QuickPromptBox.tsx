/* eslint-disable i18next/no-literal-string */
import React, { useEffect, useRef, useState, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { commands } from "@/bindings";
import type { PromptHistoryEntry } from "@/bindings";
import { toast } from "sonner";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";

/**
 * Spotlight / Raycast-style prompt box.
 * Lightweight: plain textarea + filtered history, no Tiptap.
 * Global shortcut toggles the window (see shortcut handler quick_prompt).
 * Inside: Meta/Cmd+Enter (or Ctrl+Enter on Win/Linux) pastes to previous app,
 * Esc closes without pasting. Enter inserts newline (preserved).
 */
export const QuickPromptBox: React.FC = () => {
  const { t } = useTranslation();
  const [text, setText] = useState("");
  const [history, setHistory] = useState<PromptHistoryEntry[]>([]);
  const [selectedIdx, setSelectedIdx] = useState(-1);
  const [pasting, setPasting] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Load history
  const refreshHistory = async () => {
    try {
      const res = await commands.listPromptHistory(50);
      if (res.status === "ok") setHistory(res.data);
    } catch {}
  };

  useEffect(() => {
    refreshHistory();
    const unlisten = listen("quick-prompt:opened", () => {
      refreshHistory();
      // focus after window animation
      setTimeout(() => textareaRef.current?.focus(), 60);
    });
    // also focus on mount (window already visible)
    setTimeout(() => textareaRef.current?.focus(), 80);
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Refocus when window gains focus (e.g. toggled)
  useEffect(() => {
    const onFocus = () => textareaRef.current?.focus();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, []);

  const filtered = useMemo(() => {
    const q = text.trim().toLowerCase();
    if (!q) return history.slice(0, 6);
    return history.filter((h) => h.prompt_text.toLowerCase().includes(q)).slice(0, 6);
  }, [text, history]);

  // Keep selection in bounds
  useEffect(() => {
    if (selectedIdx >= filtered.length) setSelectedIdx(-1);
  }, [filtered.length, selectedIdx]);

  const close = async () => {
    try {
      await commands.hideQuickPromptCommand();
    } catch {}
    // fallback: hide window directly
    try {
      await getCurrentWindow().hide();
    } catch {}
  };

  const paste = async (overrideText?: string) => {
    const toPaste = (overrideText ?? text).trim();
    if (!toPaste || pasting) return;
    setPasting(true);
    try {
      const res = await commands.quickPromptPaste(toPaste);
      if (res.status === "ok") {
        setText("");
        // history will include new entry; refresh locally
        setHistory((prev) => [res.data, ...prev.filter((h) => h.id !== res.data.id)].slice(0, 50));
      } else {
        toast.error(t("promptStudio.sendFailed"), { description: res.error });
      }
    } catch (e) {
      toast.error(t("promptStudio.sendFailed"));
    } finally {
      setPasting(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    const isMeta = e.metaKey || e.ctrlKey; // Cmd on mac, Ctrl on others both work

    // Meta+Enter -> paste & return focus to previous app
    if (isMeta && e.key === "Enter") {
      e.preventDefault();
      if (selectedIdx >= 0 && filtered[selectedIdx]) {
        // if history item selected, paste that instead of textarea content?
        // Prefer textarea content; if empty use selected
        const useSelected = !text.trim();
        if (useSelected) {
          paste(filtered[selectedIdx].prompt_text);
          return;
        }
      }
      paste();
      return;
    }

    if (e.key === "Escape") {
      e.preventDefault();
      if (selectedIdx >= 0) {
        setSelectedIdx(-1);
      } else {
        close();
      }
      return;
    }

    // History navigation when dropdown visible
    if (filtered.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIdx((i) => (i + 1) % filtered.length);
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIdx((i) => (i - 1 + filtered.length) % filtered.length);
        return;
      }
      // Tab to autocomplete selected
      if (e.key === "Tab" && selectedIdx >= 0) {
        e.preventDefault();
        setText(filtered[selectedIdx].prompt_text);
        setSelectedIdx(-1);
        return;
      }
    }
  };

  const handleSelectHistory = (entry: PromptHistoryEntry) => {
    setText(entry.prompt_text);
    textareaRef.current?.focus();
  };

  const isMac = navigator.platform.toLowerCase().includes("mac");
  const metaLabel = isMac ? "⌘" : "Ctrl";
  const canPaste = text.trim().length > 0 && !pasting;

  return (
    <div className="quick-prompt-root w-screen h-screen flex items-center justify-center p-4 bg-transparent select-none">
      <div className="w-full max-w-[640px] rounded-[16px] border border-mid-gray/20 bg-background shadow-[0_16px_48px_rgba(0,0,0,0.18),0_2px_8px_rgba(0,0,0,0.12)] overflow-hidden flex flex-col">
        {/* Header / drag region */}
        <div data-tauri-drag-region className="flex items-center justify-between px-4 py-2.5 border-b border-mid-gray/10 bg-mid-gray/[0.04] cursor-grab active:cursor-grabbing select-none">
          <div className="flex items-center gap-2 text-xs font-medium text-text/60">
            <span className="w-2 h-2 rounded-full bg-logo-primary animate-pulse" aria-hidden />
            Quick Prompt
          </div>
          <button
            onClick={close}
            aria-label="Close"
            className="w-6 h-6 rounded-full bg-mid-gray/10 hover:bg-mid-gray/20 flex items-center justify-center text-text/50 hover:text-text transition-colors"
          >
            <span className="text-[14px] leading-none">×</span>
          </button>
        </div>

        {/* Textarea */}
        <div className="p-3">
          <textarea
            ref={textareaRef}
            value={text}
            onChange={(e) => {
              setText(e.target.value);
              setSelectedIdx(-1);
            }}
            onKeyDown={handleKeyDown}
            placeholder={t("quickPrompt.placeholder", { defaultValue: "Write a prompt or snippet…  (Enter for new line, {{meta}}+Enter to paste)" , meta: metaLabel })}
            rows={4}
            autoFocus
            spellCheck={false}
            className="w-full min-h-[96px] max-h-[160px] resize-none rounded-xl bg-mid-gray/[0.06] border border-mid-gray/20 px-3.5 py-3 text-[14px] leading-5 placeholder:text-text/35 focus:outline-none focus:ring-2 focus:ring-logo-primary/30 focus:border-logo-primary/40 transition-colors"
          />
        </div>

        {/* History suggestions */}
        {filtered.length > 0 && (
          <div className="px-3 pb-2">
            <div className="rounded-xl border border-mid-gray/10 bg-mid-gray/[0.03] overflow-hidden divide-y divide-mid-gray/10 max-h-[160px] overflow-y-auto">
              <div className="px-3 py-1.5 text-[10px] font-semibold tracking-widest uppercase text-text/30">Recent</div>
              {filtered.map((entry, idx) => (
                <button
                  key={entry.id}
                  onClick={() => handleSelectHistory(entry)}
                  onDoubleClick={() => paste(entry.prompt_text)}
                  className={`w-full text-left px-3 py-2 flex items-start justify-between gap-3 hover:bg-mid-gray/10 transition-colors ${idx === selectedIdx ? "bg-logo-primary/10 ring-1 ring-inset ring-logo-primary/20" : ""}`}
                >
                  <span className="text-[13px] leading-4 line-clamp-2 flex-1 whitespace-pre-wrap break-words">{entry.prompt_text}</span>
                  <span className="shrink-0 text-[10px] text-text/30 mt-0.5">
                    {new Date(entry.timestamp * 1000).toLocaleDateString()}
                  </span>
                </button>
              ))}
            </div>
            <p className="text-[11px] text-text/30 px-1 pt-1.5">↑↓ to navigate • Tab to fill • Double-click to paste instantly</p>
          </div>
        )}

        {/* Footer actions */}
        <div className="flex flex-wrap items-center justify-between gap-2 px-3 py-2.5 border-t border-mid-gray/10 bg-mid-gray/[0.02]">
          <div className="flex items-center gap-1.5 text-[11px] text-text/40">
            <kbd className="px-1.5 py-0.5 rounded border border-mid-gray/20 bg-background text-[11px] font-medium">{metaLabel}↵</kbd>
            <span>paste</span>
            <span className="mx-1 text-text/20">·</span>
            <kbd className="px-1.5 py-0.5 rounded border border-mid-gray/20 bg-background text-[11px]">Esc</kbd>
            <span>close</span>
            <span className="mx-1 text-text/20">·</span>
            <span>↵ new line</span>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={close}
              className="inline-flex items-center justify-center px-4 py-1.5 rounded-full text-xs font-medium border border-mid-gray/20 bg-background hover:bg-mid-gray/10 transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={() => paste()}
              disabled={!canPaste}
              className="inline-flex items-center justify-center px-4 py-1.5 rounded-full text-xs font-semibold border border-transparent bg-logo-primary text-white hover:brightness-105 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            >
              {pasting ? "Pasting…" : `Paste ${metaLabel}↵`}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
