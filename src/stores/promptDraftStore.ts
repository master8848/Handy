import { create } from "zustand";

interface PromptDraftState {
  draft: string | null;
  setDraft: (text: string) => void;
  consumeDraft: () => string | null;
}

// One-shot handoff from the prompt history page ("reuse") back to the Prompt
// Studio textarea. The draft is consumed on mount, so stale reuse requests
// never re-populate the box.
export const usePromptDraftStore = create<PromptDraftState>((set, get) => ({
  draft: null,
  setDraft: (text) => set({ draft: text }),
  consumeDraft: () => {
    const draft = get().draft;
    set({ draft: null });
    return draft;
  },
}));
