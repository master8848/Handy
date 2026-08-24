import { create } from "zustand";

interface WorkbenchDraftState {
  /** Last editor HTML for the Prompt Workbench, kept across tab switches. */
  draftHtml: string | null;
  setDraftHtml: (html: string) => void;
}

// Session-scoped persistence for the Prompt Workbench draft. The workbench
// unmounts when switching tabs/windows content; this keeps the document so the
// user's prompt survives navigation (separate from the dictation box draft).
export const useWorkbenchDraftStore = create<WorkbenchDraftState>((set) => ({
  draftHtml: null,
  setDraftHtml: (html) => set({ draftHtml: html }),
}));
