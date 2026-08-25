import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { commands } from "@/bindings";
import type { PromptVersion } from "@/bindings";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";

interface Props {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  promptId: number | null;
  onRestored: () => void;
}

export const VersionHistoryDrawer: React.FC<Props> = ({ open, onOpenChange, promptId, onRestored }) => {
  const { t } = useTranslation();
  const [versions, setVersions] = useState<PromptVersion[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (open && promptId) {
      setLoading(true);
      commands
        .listPromptVersions(promptId)
        .then((res) => {
          if (res.status === "ok") setVersions(res.data);
        })
        .finally(() => setLoading(false));
    }
  }, [open, promptId]);

  const handleRestore = async (v: number) => {
    if (!promptId) return;
    const res = await commands.restorePromptVersion(promptId, v);
    if (res.status === "error") toast.error(res.error);
    else {
      toast.success(t("promptLibrary.restored"));
      onRestored();
      onOpenChange(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange} title={t("promptLibrary.versionHistory")} closeLabel={t("common.close")}>
      <div className="space-y-2 max-h-[60vh] overflow-y-auto">
        {loading ? (
          <p className="text-sm text-text/60">{t("common.loading")}</p>
        ) : versions.length === 0 ? (
          <p className="text-sm text-text/60">{t("promptLibrary.noVersions")}</p>
        ) : (
          versions.map((ver) => (
            <div key={ver.id} className="p-3 rounded border border-mid-gray/20 space-y-1">
              <div className="flex justify-between items-center">
                {/* eslint-disable-next-line i18next/no-literal-string */}
                <span className="text-sm font-medium">v{ver.version}</span>
                <Button size="sm" variant="ghost" onClick={() => handleRestore(ver.version)}>
                  {t("promptLibrary.restore")}
                </Button>
              </div>
              <p className="text-xs font-medium">{ver.title}</p>
              <p className="text-xs text-text/70 whitespace-pre-wrap break-words line-clamp-3">{ver.content}</p>
              <p className="text-[10px] text-text/50">{new Date(ver.created_at * 1000).toLocaleString()}</p>
            </div>
          ))
        )}
      </div>
    </Dialog>
  );
};
