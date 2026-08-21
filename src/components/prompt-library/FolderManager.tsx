import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { Trash2 } from "lucide-react";
import { toast } from "sonner";
import { commands } from "@/bindings";
import type { Folder } from "@/bindings";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";

interface Props {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  folders: Folder[];
  onChanged: () => void;
}

export const FolderManager: React.FC<Props> = ({ open, onOpenChange, folders, onChanged }) => {
  const { t } = useTranslation();
  const [name, setName] = useState("");
  const [color, setColor] = useState("#ec4899");

  const handleCreate = async () => {
    if (!name.trim()) return;
    const res = await commands.createFolder(name.trim(), color);
    if (res.status === "error") toast.error(res.error);
    else {
      setName("");
      onChanged();
    }
  };

  const handleDelete = async (id: number) => {
    const res = await commands.deleteFolder(id);
    if (res.status === "error") toast.error(res.error);
    else onChanged();
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange} title={t("promptLibrary.manageFolders")} closeLabel={t("common.close")}>
      <div className="space-y-3">
        <div className="flex gap-2">
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={t("promptLibrary.folderNamePlaceholder")}
            className="flex-1 px-3 py-2 text-sm bg-mid-gray/10 border border-mid-gray/40 rounded-lg focus:outline-none focus:ring-1 focus:ring-logo-primary"
          />
          <input type="color" value={color} onChange={(e) => setColor(e.target.value)} className="w-10 h-10 rounded" />
          <Button size="sm" onClick={handleCreate}>
            {t("common.add")}
          </Button>
        </div>
        <div className="space-y-1 max-h-[40vh] overflow-y-auto">
          {folders.map((f) => (
            <div key={f.id} className="flex items-center justify-between p-2 rounded border border-mid-gray/20">
              <div className="flex items-center gap-2">
                <span className="w-3 h-3 rounded-full" style={{ background: f.color ?? "#ccc" }} />
                <span className="text-sm">{f.name}</span>
              </div>
              {f.name !== "Imported" && (
                <button onClick={() => handleDelete(f.id)} className="p-1 rounded hover:bg-mid-gray/20 text-text/60">
                  <Trash2 className="w-4 h-4" />
                </button>
              )}
            </div>
          ))}
        </div>
      </div>
    </Dialog>
  );
};
