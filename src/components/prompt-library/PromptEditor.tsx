import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { commands } from "@/bindings";
import type { Prompt, Folder } from "@/bindings";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";

interface Props {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  prompt: Prompt | null;
  folders: Folder[];
  onSaved: () => void;
}

export const PromptEditor: React.FC<Props> = ({ open, onOpenChange, prompt, folders, onSaved }) => {
  const { t } = useTranslation();
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [folderId, setFolderId] = useState<string>("");
  const [tagsStr, setTagsStr] = useState("");
  const [variables, setVariables] = useState<string[]>([]);

  useEffect(() => {
    if (prompt) {
      setTitle(prompt.title);
      setContent(prompt.content);
      setFolderId(prompt.folder_id ? String(prompt.folder_id) : "");
      setTagsStr(prompt.tags.join(", "));
    } else {
      setTitle("");
      setContent("");
      setFolderId("");
      setTagsStr("");
    }
  }, [prompt, open]);

  useEffect(() => {
    const re = /\{\{\s*([A-Za-z0-9_]+)\s*\}\}/g;
    const vars: string[] = [];
    const seen = new Set<string>();
    let m: RegExpExecArray | null;
    while ((m = re.exec(content)) !== null) {
      const v = m[1];
      if (!seen.has(v)) {
        seen.add(v);
        vars.push(v);
      }
    }
    setVariables(vars);
  }, [content]);

  const handleSave = async () => {
    const fid = folderId ? Number(folderId) : null;
    const tags = tagsStr
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
    let res;
    if (prompt) {
      res = await commands.updatePrompt(prompt.id, title, content, fid, tags);
    } else {
      res = await commands.createPrompt(title, content, fid, tags);
    }
    if (res.status === "error") {
      toast.error(res.error);
    } else {
      toast.success(t("promptLibrary.saved"));
      onSaved();
      onOpenChange(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={prompt ? t("promptLibrary.editPrompt") : t("promptLibrary.newPrompt")}
      closeLabel={t("common.close")}
      footer={
        <>
          <Button variant="secondary" size="sm" onClick={() => onOpenChange(false)}>
            {t("common.cancel")}
          </Button>
          <Button size="sm" onClick={handleSave}>
            {t("common.save")}
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <div className="space-y-1">
          <label className="text-sm font-medium">{t("promptLibrary.fieldTitle")}</label>
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder={t("promptLibrary.titlePlaceholder")}
            className="w-full px-3 py-2 text-sm bg-mid-gray/10 border border-mid-gray/40 rounded-lg focus:outline-none focus:ring-1 focus:ring-logo-primary"
          />
        </div>
        <div className="space-y-1">
          <label className="text-sm font-medium">{t("promptLibrary.folder")}</label>
          <select
            value={folderId}
            onChange={(e) => setFolderId(e.target.value)}
            className="w-full px-3 py-2 text-sm bg-mid-gray/10 border border-mid-gray/40 rounded-lg"
          >
            <option value="">{t("promptLibrary.noFolder")}</option>
            {folders.map((f) => (
              <option key={f.id} value={String(f.id)}>
                {f.name}
              </option>
            ))}
          </select>
        </div>
        <div className="space-y-1">
          <label className="text-sm font-medium">{t("promptLibrary.fieldContent")}</label>
          <textarea
            value={content}
            onChange={(e) => setContent(e.target.value)}
            placeholder={t("promptLibrary.contentPlaceholder")}
            rows={6}
            className="w-full px-3 py-2 text-sm bg-mid-gray/10 border border-mid-gray/40 rounded-lg focus:outline-none focus:ring-1 focus:ring-logo-primary"
          />
          {variables.length > 0 && (
            <div className="flex flex-wrap gap-1">
              {variables.map((v) => (
                <span key={v} className="text-xs px-2 py-0.5 rounded bg-amber-500/20">
                  {`{{${v}}}`}
                </span>
              ))}
            </div>
          )}
        </div>
        <div className="space-y-1">
          <label className="text-sm font-medium">{t("promptLibrary.fieldTags")}</label>
          <input
            value={tagsStr}
            onChange={(e) => setTagsStr(e.target.value)}
            placeholder={t("promptLibrary.tagsPlaceholder")}
            className="w-full px-3 py-2 text-sm bg-mid-gray/10 border border-mid-gray/40 rounded-lg focus:outline-none focus:ring-1 focus:ring-logo-primary"
          />
        </div>
      </div>
    </Dialog>
  );
};
