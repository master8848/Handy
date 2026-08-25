import React, { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { FileAudio, Loader2, Copy, Check, FileText } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import { commands, type ModelInfo } from "@/bindings";
import { useSettings } from "../../hooks/useSettings";

type Phase =
  | "idle"
  | "decoding"
  | "loading_model"
  | "transcribing"
  | "finalizing";

const AUDIO_FILTERS = [
  {
    name: "Audio",
    extensions: ["mp3", "wav", "m4a", "aac", "ogg", "flac", "oga", "mp4"],
  },
  { name: "All Files", extensions: ["*"] },
];

export const TranscribeFiles: React.FC = () => {
  const { t } = useTranslation();
  const { settings } = useSettings();

  const [phase, setPhase] = useState<Phase>("idle");
  const [fileName, setFileName] = useState<string | null>(null);
  const [result, setResult] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [isDragging, setIsDragging] = useState(false);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [selectedModel, setSelectedModel] = useState<string | null>(null);
  const busyRef = useRef(false);

  useEffect(() => {
    commands.getAvailableModels().then((r) => {
      if (r.status === "ok") {
        setModels(r.data);
        setSelectedModel(
          (cur) => cur ?? r.data.find((m) => m.is_downloaded)?.id ?? null,
        );
      }
    });
  }, []);

  useEffect(() => {
    const unlisten = listen<string>("file-transcription-status", (event) => {
      const next = event.payload as Phase;
      if (
        next === "decoding" ||
        next === "loading_model" ||
        next === "finalizing"
      ) {
        setPhase(next);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const transcribe = useCallback(
    async (path: string, displayName: string) => {
      if (busyRef.current) return;
      busyRef.current = true;
      setResult(null);
      setError(null);
      setCopied(false);
      setFileName(displayName);
      setPhase("decoding");

      const result = await commands.transcribeAudioFile(path, selectedModel);
      if (result.status === "ok") {
        setResult(result.data);
        setPhase("idle");
      } else {
        setError(result.error);
        setPhase("idle");
      }
      busyRef.current = false;
    },
    [selectedModel],
  );

  const handlePick = useCallback(async () => {
    const path = await open({
      multiple: false,
      directory: false,
      filters: AUDIO_FILTERS,
    });
    if (typeof path !== "string") return;
    const name = path.split("/").pop()?.split("\\").pop() ?? path;
    transcribe(path, name);
  }, [transcribe]);

  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "over") {
        setIsDragging(true);
      } else if (event.payload.type === "leave") {
        setIsDragging(false);
      } else if (event.payload.type === "drop") {
        setIsDragging(false);
        const path = event.payload.paths[0];
        if (path) {
          const name = path.split("/").pop()?.split("\\").pop() ?? path;
          transcribe(path, name);
        }
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [transcribe]);

  const copyResult = useCallback(async () => {
    if (!result) return;
    await navigator.clipboard.writeText(result);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [result]);

  const phaseLabel = (p: Phase): string | null => {
    switch (p) {
      case "decoding":
        return t("transcribe.phase.decoding");
      case "loading_model":
        return t("transcribe.phase.loadingModel");
      case "finalizing":
        return t("transcribe.phase.finalizing");
      default:
        return null;
    }
  };

  const modelName =
    models.find((m) => m.id === selectedModel)?.name ??
    settings?.selected_model ??
    "";

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <div
        onDragOver={(e) => e.preventDefault()}
        className={`w-full rounded-xl border-2 border-dashed p-8 flex flex-col items-center gap-3 text-center transition-colors cursor-pointer ${
          isDragging
            ? "border-logo-primary bg-logo-primary/10"
            : "border-mid-gray/30 hover:border-mid-gray/60"
        }`}
        onClick={handlePick}
      >
        {phase === "idle" && !result && !error && (
          <>
            <FileAudio size={40} className="text-mid-gray" />
            <p className="text-sm font-medium">{t("transcribe.dropTitle")}</p>
            <p className="text-xs text-mid-gray">
              {t("transcribe.dropHint", {
                formats: "MP3, WAV, M4A, OGG, FLAC",
              })}
            </p>
            <p className="text-xs text-logo-primary mt-1">
              {t("transcribe.browseFiles")}
            </p>
          </>
        )}

        {phase !== "idle" && (
          <div className="flex flex-col items-center gap-3">
            <Loader2 size={36} className="animate-spin text-logo-primary" />
            {fileName && (
              <p className="text-sm font-medium break-all max-w-md">
                {fileName}
              </p>
            )}
            <p className="text-xs text-mid-gray">
              {phaseLabel(phase)} {modelName && `· ${modelName}`}
            </p>
          </div>
        )}

        {phase === "idle" && error && (
          <div className="flex flex-col items-center gap-2">
            <p className="text-sm font-medium text-red-400 break-all max-w-md">
              {error}
            </p>
            <p className="text-xs text-mid-gray">{t("transcribe.tryAgain")}</p>
          </div>
        )}
      </div>

      {phase === "idle" && result && (
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <p className="text-sm font-medium flex items-center gap-2">
              <FileText size={16} />
              {t("transcribe.result")}
            </p>
            <button
              onClick={copyResult}
              className="flex items-center gap-1 px-3 py-1.5 rounded-md bg-logo-primary/80 hover:bg-logo-primary text-xs font-medium"
            >
              {copied ? <Check size={14} /> : <Copy size={14} />}
              {copied ? t("transcribe.copied") : t("transcribe.copy")}
            </button>
          </div>
          <textarea
            readOnly
            value={result}
            className="w-full h-64 p-3 rounded-lg bg-mid-gray/10 border border-mid-gray/20 text-sm font-mono resize-y"
          />
        </div>
      )}
    </div>
  );
};
