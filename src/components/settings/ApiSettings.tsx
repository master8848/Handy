import React, { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { commands } from "@/bindings";
import { Button } from "../ui/Button";
import { Input } from "../ui/Input";
import { Select } from "../ui/Select";
import { SettingContainer } from "../ui/SettingContainer";
import { ToggleSwitch } from "../ui/ToggleSwitch";

function docsUrl(port: number): string {
  return `http://127.0.0.1:${port}/openapi.json`;
}

export const ApiSettings: React.FC<{ grouped?: boolean }> = ({ grouped = false }) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating, refreshSettings } = useSettings();
  const port = (getSetting("server_port") as number) ?? 17373;
  const token = (getSetting("server_auth_token") as string | null) ?? null;
  const policy = (getSetting("api_model_load_policy") as string) ?? "auto_allow";
  const lazy = (getSetting("api_lazy_transcribe") as boolean) ?? true;
  const [copied, setCopied] = useState<string | null>(null);
  const [regenLoading, setRegenLoading] = useState(false);

  const baseUrl = useMemo(() => `http://127.0.0.1:${port}`, [port]);
  const maskedToken = token ?? "";

  const copy = async (text: string, key: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(key);
      setTimeout(() => setCopied(null), 1500);
    } catch {
      // ignore
    }
  };

  const handleRegenerate = async () => {
    setRegenLoading(true);
    try {
      const res = await commands.regenerateServerTokenSetting();
      if (res.status === "ok") await refreshSettings();
    } finally {
      setRegenLoading(false);
    }
  };

  const curlOpenAI = `curl -X POST ${baseUrl}/v1/audio/transcriptions \\\n  -H "Authorization: Bearer ${maskedToken || "TOKEN"}" \\\n  -F file=@audio.wav \\\n  -F model="whisper-small"`;

  const curlNative = `curl -X POST ${baseUrl}/api/transcribe \\\n  -H "Authorization: Bearer ${maskedToken || "TOKEN"}" \\\n  -F file=@audio.wav \\\n  -F model="whisper-small"`;

  const jsOpenAI = `import OpenAI from "openai";\nimport fs from "fs";\nconst openai = new OpenAI({ baseURL: "${baseUrl}/v1", apiKey: "${maskedToken || "TOKEN"}" });\nconst transcription = await openai.audio.transcriptions.create({\n  file: fs.createReadStream("audio.wav"),\n  model: "whisper-small",\n});\nconsole.log(transcription.text);`;

  const jsFetch = `const form = new FormData();\nform.append("file", file); // File/Blob\nform.append("model", "whisper-small");\nconst res = await fetch("${baseUrl}/v1/audio/transcriptions", {\n  method: "POST",\n  headers: { Authorization: "Bearer ${maskedToken || "TOKEN"}" },\n  body: form,\n});\nconst data = await res.json();\nconsole.log(data.text);`;

  const pythonOpenAI = `from openai import OpenAI\nclient = OpenAI(base_url="${baseUrl}/v1", api_key="${maskedToken || "TOKEN"}")\nwith open("audio.wav", "rb") as f:\n    resp = client.audio.transcriptions.create(model="whisper-small", file=f)\n    print(resp.text)`;

  const pythonRequests = `import requests\nwith open("audio.wav", "rb") as f:\n    r = requests.post("${baseUrl}/v1/audio/transcriptions",\n        headers={"Authorization": "Bearer ${maskedToken || "TOKEN"}"},\n        files={"file": f}, data={"model": "whisper-small"})\n    print(r.json()["text"])`;

  return (
    <div className="space-y-3">
      <SettingContainer
        title={t("settings.advanced.api.title")}
        description={t("settings.advanced.api.description")}
        descriptionMode="inline"
        grouped={grouped}
      >
        <div className="w-full text-xs text-mid-gray">{t("settings.advanced.api.osSpeechHelp")}</div>
      </SettingContainer>

      <SettingContainer
        title={t("settings.advanced.api.policy.title")}
        description={t("settings.advanced.api.policy.description")}
        descriptionMode="tooltip"
        grouped={grouped}
      >
        <Select
          value={policy}
          onChange={(v) => {
            if (v) updateSetting("api_model_load_policy" as never, v as never);
          }}
          disabled={isUpdating("api_model_load_policy")}
          isClearable={false}
          options={[
            { value: "auto_allow", label: t("settings.advanced.api.policy.options.auto_allow") },
            { value: "always_ask", label: t("settings.advanced.api.policy.options.always_ask") },
            { value: "never", label: t("settings.advanced.api.policy.options.never") },
          ]}
          className="w-64"
        />
      </SettingContainer>

      <ToggleSwitch
        checked={lazy}
        onChange={(v) => updateSetting("api_lazy_transcribe" as never, v as never)}
        isUpdating={isUpdating("api_lazy_transcribe")}
        label={t("settings.advanced.api.lazy.title")}
        description={t("settings.advanced.api.lazy.description")}
        descriptionMode="tooltip"
        grouped={grouped}
      />

      <SettingContainer
        title={t("settings.advanced.api.baseUrl")}
        description={`${baseUrl} — ${t("settings.advanced.api.tokenHint")}`}
        descriptionMode="tooltip"
        grouped={grouped}
      >
        <div className="flex items-center gap-2 w-full">
          <Input value={baseUrl} readOnly className="flex-1 font-mono text-xs" />
          <Button variant="secondary" size="sm" onClick={() => copy(baseUrl, "base")}>
            {copied === "base" ? t("settings.advanced.api.copied") : t("common.copy")}
          </Button>
        </div>
      </SettingContainer>

      <SettingContainer
        title={t("settings.advanced.serverMode.tokenLabel")}
        description={t("settings.advanced.api.tokenHint")}
        descriptionMode="tooltip"
        grouped={grouped}
      >
        <div className="flex items-center gap-2 w-full">
          <Input value={token ?? ""} readOnly placeholder="—" className="flex-1 font-mono text-xs" />
          <Button variant="secondary" size="sm" onClick={() => token && copy(token, "token")} disabled={!token}>
            {copied === "token" ? t("settings.advanced.api.copied") : t("settings.advanced.serverMode.copyToken")}
          </Button>
          <Button variant="secondary" size="sm" onClick={handleRegenerate} disabled={regenLoading}>
            {t("settings.advanced.serverMode.regenerate")}
          </Button>
        </div>
      </SettingContainer>

      <SettingContainer title={t("settings.advanced.api.openDocs")} description={docsUrl(port)} descriptionMode="inline" grouped={grouped}>
        <a
          href={docsUrl(port)}
          target="_blank"
          rel="noreferrer"
          className="text-xs underline text-logo-primary hover:text-logo-primary/80 break-all"
        >
          {docsUrl(port)}
        </a>
      </SettingContainer>

      <div className="rounded-md border border-mid-gray/20 p-3 space-y-3">
        <h4 className="text-sm font-medium">Examples — lazy (no preload required)</h4>

        <div className="space-y-1">
          <div className="flex items-center justify-between">
            <span className="text-xs font-mono text-mid-gray">{t("settings.advanced.api.examples.openai")}</span>
            <Button variant="secondary" size="sm" onClick={() => copy(curlOpenAI, "curl-openai")}>
              {copied === "curl-openai" ? t("settings.advanced.api.copied") : t("settings.advanced.api.copyCurl")}
            </Button>
          </div>
          <pre className="text-xs bg-mid-gray/10 rounded p-2 overflow-auto whitespace-pre-wrap break-words">{curlOpenAI}</pre>
        </div>

        <div className="space-y-1">
          <div className="flex items-center justify-between">
            <span className="text-xs font-mono text-mid-gray">{t("settings.advanced.api.examples.native")}</span>
            <Button variant="secondary" size="sm" onClick={() => copy(curlNative, "curl-native")}>
              {copied === "curl-native" ? t("settings.advanced.api.copied") : t("settings.advanced.api.copyCurl")}
            </Button>
          </div>
          <pre className="text-xs bg-mid-gray/10 rounded p-2 overflow-auto whitespace-pre-wrap break-words">{curlNative}</pre>
        </div>

        <div className="space-y-1">
          <div className="flex items-center justify-between">
            <span className="text-xs font-mono text-mid-gray">JavaScript (OpenAI SDK)</span>
            <Button variant="secondary" size="sm" onClick={() => copy(jsOpenAI, "js-openai")}>
              {copied === "js-openai" ? t("settings.advanced.api.copied") : t("settings.advanced.api.copyJs")}
            </Button>
          </div>
          <pre className="text-xs bg-mid-gray/10 rounded p-2 overflow-auto whitespace-pre-wrap break-words">{jsOpenAI}</pre>
        </div>

        <div className="space-y-1">
          <div className="flex items-center justify-between">
            <span className="text-xs font-mono text-mid-gray">JavaScript (fetch)</span>
            <Button variant="secondary" size="sm" onClick={() => copy(jsFetch, "js-fetch")}>
              {copied === "js-fetch" ? t("settings.advanced.api.copied") : t("settings.advanced.api.copyJs")}
            </Button>
          </div>
          <pre className="text-xs bg-mid-gray/10 rounded p-2 overflow-auto whitespace-pre-wrap break-words">{jsFetch}</pre>
        </div>

        <div className="space-y-1">
          <div className="flex items-center justify-between">
            <span className="text-xs font-mono text-mid-gray">Python (OpenAI SDK)</span>
            <Button variant="secondary" size="sm" onClick={() => copy(pythonOpenAI, "py-openai")}>
              {copied === "py-openai" ? t("settings.advanced.api.copied") : t("settings.advanced.api.copyPython")}
            </Button>
          </div>
          <pre className="text-xs bg-mid-gray/10 rounded p-2 overflow-auto whitespace-pre-wrap break-words">{pythonOpenAI}</pre>
        </div>

        <div className="space-y-1">
          <div className="flex items-center justify-between">
            <span className="text-xs font-mono text-mid-gray">Python (requests)</span>
            <Button variant="secondary" size="sm" onClick={() => copy(pythonRequests, "py-req")}>
              {copied === "py-req" ? t("settings.advanced.api.copied") : t("settings.advanced.api.copyPython")}
            </Button>
          </div>
          <pre className="text-xs bg-mid-gray/10 rounded p-2 overflow-auto whitespace-pre-wrap break-words">{pythonRequests}</pre>
        </div>

        <div className="text-xs text-mid-gray space-y-1">
          <div>{t("settings.advanced.api.examples.listModels")}: <code className="font-mono bg-mid-gray/10 px-1 rounded">GET {baseUrl}/v1/models</code> (or <code className="font-mono bg-mid-gray/10 px-1 rounded">/api/models</code>)</div>
          <div>{t("settings.advanced.api.examples.modelStatus")}: <code className="font-mono bg-mid-gray/10 px-1 rounded">GET {baseUrl}/api/model/status</code></div>
          <div>{t("settings.advanced.api.examples.loadModel")}: <code className="font-mono bg-mid-gray/10 px-1 rounded">POST {baseUrl}/api/model/load</code> {"{"}"model":"whisper-small"{"}"} </div>
          <div>Translations: <code className="font-mono bg-mid-gray/10 px-1 rounded">POST {baseUrl}/v1/audio/translations</code> (forces translate)</div>
          <div><code className="font-mono bg-mid-gray/10 px-1 rounded">os-speech</code> is the OS built-in recognizer model id — no download needed; use it as <code className="font-mono bg-mid-gray/10 px-1 rounded">model=os-speech</code>.</div>
        </div>
      </div>
    </div>
  );
};
