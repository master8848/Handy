import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Prompt } from "@/bindings";
import { Dialog } from "@/components/ui/Dialog";
import { Button } from "@/components/ui/Button";

interface Props {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  prompt: Prompt;
  onSubmit: (vars: Record<string, string>) => void;
}

export const VariableFillDialog: React.FC<Props> = ({ open, onOpenChange, prompt, onSubmit }) => {
  const { t } = useTranslation();
  const [values, setValues] = useState<Record<string, string>>({});

  useEffect(() => {
    if (open) {
      const stored: Record<string, string> = {};
      for (const v of prompt.variables) {
        const key = `prompt-var-${prompt.id}-${v}`;
        stored[v] = localStorage.getItem(key) ?? "";
      }
      setValues(stored);
    }
  }, [open, prompt]);

  const handleSubmit = () => {
    for (const [k, v] of Object.entries(values)) {
      localStorage.setItem(`prompt-var-${prompt.id}-${k}`, v);
    }
    onSubmit(values);
    onOpenChange(false);
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={t("promptLibrary.fillVariables")}
      closeLabel={t("common.close")}
      footer={
        <>
          <Button variant="secondary" size="sm" onClick={() => onOpenChange(false)}>
            {t("common.cancel")}
          </Button>
          <Button size="sm" onClick={handleSubmit}>
            {t("promptLibrary.insert")}
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        {prompt.variables.map((v) => (
          <div key={v} className="space-y-1">
            <label className="text-sm font-medium">{v}</label>
            <input
              value={values[v] ?? ""}
              onChange={(e) => setValues({ ...values, [v]: e.target.value })}
              placeholder={v}
              className="w-full px-3 py-2 text-sm bg-mid-gray/10 border border-mid-gray/40 rounded-lg focus:outline-none focus:ring-1 focus:ring-logo-primary"
            />
          </div>
        ))}
      </div>
    </Dialog>
  );
};
