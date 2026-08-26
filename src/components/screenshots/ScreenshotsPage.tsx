import React, { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, Copy, ExternalLink, ImageOff } from "lucide-react";

export interface ScreenshotEntry {
  file: string;
  titleKey: string;
  descriptionKey: string;
  width: number;
  height?: number;
}

export const SCREENSHOT_LIST: ScreenshotEntry[] = [
  {
    file: "dictate-desktop.png",
    titleKey: "screenshots.items.dictateDesktop.title",
    descriptionKey: "screenshots.items.dictateDesktop.description",
    width: 1280,
  },
  {
    file: "prompt-desktop.png",
    titleKey: "screenshots.items.promptDesktop.title",
    descriptionKey: "screenshots.items.promptDesktop.description",
    width: 1280,
  },
  {
    file: "transcribe-desktop.png",
    titleKey: "screenshots.items.transcribeDesktop.title",
    descriptionKey: "screenshots.items.transcribeDesktop.description",
    width: 1280,
  },
  {
    file: "toolbar-dropdown.png",
    titleKey: "screenshots.items.toolbarDropdown.title",
    descriptionKey: "screenshots.items.toolbarDropdown.description",
    width: 1280,
  },
  {
    file: "prompt-workbench-expanded.png",
    titleKey: "screenshots.items.promptExpanded.title",
    descriptionKey: "screenshots.items.promptExpanded.description",
    width: 1280,
  },
  {
    file: "dictation-mobile.png",
    titleKey: "screenshots.items.dictateMobile.title",
    descriptionKey: "screenshots.items.dictateMobile.description",
    width: 390,
  },
  {
    file: "prompt-mobile.png",
    titleKey: "screenshots.items.promptMobile.title",
    descriptionKey: "screenshots.items.promptMobile.description",
    width: 390,
  },
  {
    file: "transcribe-mobile.png",
    titleKey: "screenshots.items.transcribeMobile.title",
    descriptionKey: "screenshots.items.transcribeMobile.description",
    width: 390,
  },
];

const ScreenshotCard: React.FC<{
  entry: ScreenshotEntry;
  checked: boolean;
  onToggle: (file: string, checked: boolean) => void;
}> = ({ entry, checked, onToggle }) => {
  const { t } = useTranslation();
  const [missing, setMissing] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const src = `/screenshots/${entry.file}`;

  return (
    <div className="rounded-xl border border-mid-gray/20 bg-mid-gray/5 overflow-hidden flex flex-col">
      <div className="relative bg-background min-h-[180px] flex items-center justify-center overflow-hidden border-b border-mid-gray/10">
        {missing ? (
          <div className="flex flex-col items-center gap-2 py-12 px-4 text-center">
            <ImageOff className="w-8 h-8 text-text/30" />
            <p className="text-xs text-text/50 max-w-[28ch]">
              {t("screenshots.missing", { file: entry.file })}
            </p>
            <p className="text-xs text-text/40">
              {t("screenshots.runCaptureHint")}
            </p>
          </div>
        ) : (
          <>
            {!loaded && (
              <div className="absolute inset-0 grid place-items-center">
                <span className="text-xs text-text/40">
                  {t("common.loading")}
                </span>
              </div>
            )}
            <img
              src={src}
              alt={t(entry.titleKey)}
              onLoad={() => setLoaded(true)}
              onError={() => setMissing(true)}
              className="w-full h-auto object-contain max-h-[420px]"
              loading="lazy"
            />
          </>
        )}
      </div>
      <div className="p-3 space-y-2">
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0">
            <p className="text-sm font-medium leading-tight">
              {t(entry.titleKey)}
            </p>
            <p className="text-xs text-text/60 mt-0.5">
              {t(entry.descriptionKey)}
            </p>
            <p className="text-xs text-text/40 mt-1 font-mono">
              {t("screenshots.dimensions", { file: entry.file, width: entry.width })}
            </p>
          </div>
          <label className="flex items-center gap-1.5 text-xs shrink-0 cursor-pointer select-none">
            <input
              type="checkbox"
              checked={checked}
              onChange={(e) => onToggle(entry.file, e.target.checked)}
              className="w-4 h-4 rounded border-mid-gray/30"
            />
            {t("screenshots.publish")}
          </label>
        </div>
        <div className="flex gap-1.5">
          <a
            href={src}
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center gap-1 text-xs px-2 py-1 rounded-md border border-mid-gray/20 hover:bg-mid-gray/10 transition-colors"
          >
            <ExternalLink className="w-3 h-3" />
            {t("common.open")}
          </a>
          <span className="text-xs text-text/30 py-1">
            {t("screenshots.pathLabel", { file: entry.file })}
          </span>
        </div>
      </div>
    </div>
  );
};

export const ScreenshotsPage: React.FC = () => {
  const { t } = useTranslation();
  const [selected, setSelected] = useState<Record<string, boolean>>(() => {
    const init: Record<string, boolean> = {};
    for (const e of SCREENSHOT_LIST) init[e.file] = true;
    return init;
  });
  const [copied, setCopied] = useState(false);

  const handleToggle = useCallback((file: string, checked: boolean) => {
    setSelected((prev) => ({ ...prev, [file]: checked }));
  }, []);

  const markdownTable = React.useMemo(() => {
    const picked = SCREENSHOT_LIST.filter((e) => selected[e.file]);
    if (picked.length === 0) return t("screenshots.emptySelection");
    const header = `| ${t("screenshots.table.preview")} | ${t("screenshots.table.file")} | ${t("screenshots.table.description")} |\n| --- | --- | --- |`;
    const rows = picked
      .map(
        (e) =>
          `| ![${t(e.titleKey)}](screenshots/${e.file}) | \`${e.file}\` | ${t(e.descriptionKey)} |`,
      )
      .join("\n");
    return `${header}\n${rows}`;
  }, [selected, t]);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(markdownTable);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // ignore
    }
  }, [markdownTable]);

  const handleSelectAll = useCallback((value: boolean) => {
    const next: Record<string, boolean> = {};
    for (const e of SCREENSHOT_LIST) next[e.file] = value;
    setSelected(next);
  }, []);

  return (
    <div className="max-w-6xl w-full mx-auto p-6 space-y-6">
      <div className="space-y-1">
        <h1 className="text-xl font-semibold">{t("screenshots.title")}</h1>
        <p className="text-sm text-text/60">{t("screenshots.description")}</p>
        <p className="inline-flex items-center gap-2 text-xs px-2 py-1 rounded-full bg-warning/10 border border-warning/20 text-warning mt-2">
          {t("screenshots.experimentalBadge")}
        </p>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          onClick={() => handleSelectAll(true)}
          className="text-xs px-2.5 py-1.5 rounded-lg border border-mid-gray/20 hover:bg-mid-gray/10"
        >
          {t("screenshots.selectAll")}
        </button>
        <button
          type="button"
          onClick={() => handleSelectAll(false)}
          className="text-xs px-2.5 py-1.5 rounded-lg border border-mid-gray/20 hover:bg-mid-gray/10"
        >
          {t("screenshots.selectNone")}
        </button>
        <div className="flex-1" />
        <button
          type="button"
          onClick={handleCopy}
          className="inline-flex items-center gap-1.5 text-xs px-3 py-1.5 rounded-lg bg-logo-primary/15 text-logo-primary hover:bg-logo-primary/20 border border-logo-primary/20 transition-colors"
        >
          {copied ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
          {copied ? t("screenshots.copied") : t("screenshots.copyMarkdown")}
        </button>
      </div>

      <div className="rounded-lg border border-mid-gray/20 bg-mid-gray/5 p-3">
        <p className="text-xs font-medium mb-1">{t("screenshots.markdownPreview")}</p>
        <pre className="text-xs font-mono whitespace-pre-wrap break-words bg-background rounded-md p-3 border border-mid-gray/10 max-h-[220px] overflow-auto">
          {markdownTable}
        </pre>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {SCREENSHOT_LIST.map((entry) => (
          <ScreenshotCard
            key={entry.file}
            entry={entry}
            checked={!!selected[entry.file]}
            onToggle={handleToggle}
          />
        ))}
      </div>

      <div className="rounded-lg border border-mid-gray/20 bg-mid-gray/5 p-4 text-xs text-text/60 space-y-1">
        <p className="font-medium text-text/80">{t("screenshots.howToTitle")}</p>
        <p>
          {t("screenshots.howToCaptureLine", { command: "bun run screenshots:capture" })}
        </p>
        <p>
          {t("screenshots.howToViewLine", { url: "http://localhost:5173/?view=screenshots" })}
        </p>
        <p>{t("screenshots.howToPublish")}</p>
      </div>
    </div>
  );
};
