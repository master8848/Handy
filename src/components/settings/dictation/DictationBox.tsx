import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Loader2, Mic } from "lucide-react";
import { toast } from "sonner";
import { commands } from "@/bindings";
import { useModelStore } from "@/stores/modelStore";
import { useSettings } from "@/hooks/useSettings";
import { Button } from "@/components/ui/Button";

type DictationPhase = "idle" | "recording" | "transcribing";

const formatElapsed = (seconds: number): string => {
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}:${String(secs).padStart(2, "0")}`;
};

export const DictationBox: React.FC = () => {
  const { t } = useTranslation();
  const { settings, updateSetting } = useSettings();
  const { currentModel, models, selectModel } = useModelStore();

  const [text, setText] = useState("");
  const [phase, setPhase] = useState<DictationPhase>("idle");
  const [error, setError] = useState<string | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const [osSpeechAvailable, setOsSpeechAvailable] = useState(false);
  const [grantingPermission, setGrantingPermission] = useState(false);

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const selectionStartRef = useRef(0);
  const selectionEndRef = useRef(0);

  const hasUsableModel =
    !!currentModel && models.some((model) => model.is_downloaded);

  // Auto-grow the textarea with content, capped at ~40vh
  useEffect(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;
    textarea.style.height = "auto";
    textarea.style.height = `${Math.min(
      textarea.scrollHeight,
      window.innerHeight * 0.4,
    )}px`;
  }, [text]);

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

  const insertText = (transcript: string) => {
    const start = selectionStartRef.current;
    const end = selectionEndRef.current;
    setText((prev) => prev.slice(0, start) + transcript + prev.slice(end));
    const cursor = start + transcript.length;
    setTimeout(() => {
      const textarea = textareaRef.current;
      if (textarea) {
        textarea.focus();
        textarea.setSelectionRange(cursor, cursor);
      }
    }, 0);
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
            ? t("dictation.empty")
            : t("dictation.micError", { error: result.error }),
        );
      }
      setPhase("idle");
      return;
    }

    const textarea = textareaRef.current;
    const cursor = textarea?.selectionStart ?? text.length;
    selectionStartRef.current = cursor;
    selectionEndRef.current = textarea?.selectionEnd ?? cursor;
    setError(null);
    const result = await commands.startDictation();
    if (result.status === "error") {
      setError(t("dictation.micError", { error: result.error }));
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
      await navigator.clipboard.writeText(text);
      toast.success(t("dictation.copied"));
    } catch (err) {
      console.error("Failed to copy text:", err);
    }
  };

  const handleClear = () => {
    setText("");
    textareaRef.current?.focus();
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

  return (
    <div className="max-w-3xl w-full mx-auto space-y-4">
      <div className="mb-4">
        <h1 className="text-xl font-semibold mb-2">{t("dictation.title")}</h1>
        <p className="text-sm text-text/60">{t("dictation.description")}</p>
      </div>

      {/* No usable model hint — the OS speech engine needs no download */}
      {!hasUsableModel && (
        <div className="flex items-center justify-between gap-3 p-3 rounded-lg border border-warning/40 bg-warning/10">
          <div className="space-y-1">
            <p className="text-sm font-medium text-text/80">
              {t("dictation.noModel")}
            </p>
            <p className="text-sm text-text/60">
              {t("dictation.noModelDescription")}
            </p>
          </div>
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
                t("dictation.useOnDevice")
              )}
            </Button>
          )}
        </div>
      )}

      <textarea
        ref={textareaRef}
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder={t("dictation.placeholder")}
        style={{ resize: "none" }}
        className="w-full px-3 py-2 min-h-[100px] text-sm font-semibold bg-mid-gray/10 border border-mid-gray/80 rounded-md text-start transition-[background-color,border-color] duration-150 hover:bg-logo-primary/10 hover:border-logo-primary focus:outline-none focus:bg-logo-primary/10 focus:border-logo-primary overflow-y-auto"
      />

      {error && <p className="text-sm text-red-400">{error}</p>}

      <div className="flex items-center gap-2 flex-wrap">
        <Button
          variant={phase === "recording" ? "danger" : "primary"}
          size="sm"
          onClick={handleMicClick}
          disabled={phase === "transcribing"}
          className={phase === "recording" ? "animate-pulse" : ""}
        >
          <Mic className="w-3.5 h-3.5" />
          {phase === "recording"
            ? t("dictation.stopRecording")
            : t("dictation.startRecording")}
        </Button>

        {phase === "recording" && (
          <span className="flex items-center gap-1.5 text-sm text-red-400">
            <span className="w-2 h-2 rounded-full bg-red-500 animate-pulse" />
            {t("dictation.recording")} {formatElapsed(elapsed)}
          </span>
        )}

        {phase === "transcribing" && (
          <span className="flex items-center gap-2 text-sm text-text/60">
            <Loader2 className="w-3.5 h-3.5 animate-spin" />
            {t("dictation.transcribing")}
            <Button variant="ghost" size="sm" onClick={handleCancel}>
              {t("dictation.cancel")}
            </Button>
          </span>
        )}

        <div className="flex-1" />

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
          {t("dictation.spellCheck")}
        </label>

        <Button
          variant="secondary"
          size="sm"
          onClick={handleCopy}
          disabled={!text}
        >
          {t("common.copy")}
        </Button>

        <Button
          variant="secondary"
          size="sm"
          onClick={handleClear}
          disabled={!text}
        >
          {t("common.clear")}
        </Button>
      </div>
    </div>
  );
};
