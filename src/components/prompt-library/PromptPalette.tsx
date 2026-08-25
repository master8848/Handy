import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { commands } from "@/bindings";
import type { Prompt } from "@/bindings";
import { Dialog } from "@/components/ui/Dialog";
import { VariableFillDialog } from "./VariableFillDialog";
import { SEARCH_DEBOUNCE_MS } from "@/lib/constants/debounce";
import { toast } from "sonner";

export const PromptPalette: React.FC = () => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [prompts, setPrompts] = useState<Prompt[]>([]);
  const [selected, setSelected] = useState<Prompt | null>(null);
  const [showVars, setShowVars] = useState(false);
  const debounceRef = useRef<number | null>(null);

  useEffect(() => {
    const openPalette = () => {
      setOpen(true);
      commands
        .listPrompts({
          search: null,
          folder_id: null,
          tag: null,
          pinned_only: null,
          sort: "updatedDesc",
          limit: null,
          offset: null,
        })
        .then((res) => {
          if (res.status === "ok") setPrompts(res.data);
        });
    };
    // Backend event (global shortcut) plus a same-window DOM event so views
    // like the Prompt Workbench can open the palette without round-tripping
    // through Rust.
    const unlisten = listen("prompt-library:open-palette", openPalette);
    window.addEventListener("handy:open-palette", openPalette);
    return () => {
      unlisten.then((fn) => fn());
      window.removeEventListener("handy:open-palette", openPalette);
    };
  }, []);

  useEffect(() => {
    if (!open) return;
    if (debounceRef.current) window.clearTimeout(debounceRef.current);
    debounceRef.current = window.setTimeout(async () => {
      const q = query.trim();
      if (q) {
        const res = await commands.searchPrompts(q, null, 50);
        if (res.status === "ok") setPrompts(res.data);
      } else {
        const res = await commands.listPrompts({ search: null, folder_id: null, tag: null, pinned_only: null, sort: "updatedDesc", limit: null, offset: null });
        if (res.status === "ok") setPrompts(res.data);
      }
    }, SEARCH_DEBOUNCE_MS);
    return () => {
      if (debounceRef.current) window.clearTimeout(debounceRef.current);
    };
  }, [query, open]);

  const handleInsert = async (prompt: Prompt, vars?: Record<string, string>) => {
    const res = await commands.insertPrompt(prompt.id, vars ?? null);
    if (res.status === "error") toast.error(res.error);
    else {
      toast.success(t("promptLibrary.inserted"));
      setOpen(false);
    }
  };

  const onPick = (prompt: Prompt) => {
    if (prompt.variables.length > 0) {
      setSelected(prompt);
      setShowVars(true);
    } else {
      handleInsert(prompt);
    }
  };

  return (
    <>
      <Dialog open={open} onOpenChange={setOpen} title={t("promptLibrary.paletteTitle")} closeLabel={t("common.close")}>
        <div className="space-y-3">
          <input
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("promptLibrary.palettePlaceholder")}
            className="w-full px-3 py-2 text-sm bg-mid-gray/10 border border-mid-gray/40 rounded-lg focus:outline-none focus:ring-1 focus:ring-logo-primary"
          />
          <div className="max-h-[50vh] overflow-y-auto divide-y divide-mid-gray/20 rounded border border-mid-gray/20">
            {prompts.length === 0 ? (
              <p className="p-3 text-sm text-text/60">{t("promptLibrary.noResults")}</p>
            ) : (
              prompts.slice(0, 50).map((p) => (
                <button
                  key={p.id}
                  onClick={() => onPick(p)}
                  className="w-full text-left p-3 hover:bg-mid-gray/10"
                >
                  <p className="text-sm font-medium truncate">{p.title}</p>
                  <p className="text-xs text-text/60 line-clamp-2">{p.content}</p>
                </button>
              ))
            )}
          </div>
        </div>
      </Dialog>
      {selected && (
        <VariableFillDialog
          open={showVars}
          onOpenChange={setShowVars}
          prompt={selected}
          onSubmit={(vars) => handleInsert(selected, vars)}
        />
      )}
    </>
  );
};
