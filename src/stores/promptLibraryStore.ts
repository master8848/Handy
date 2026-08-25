import { create } from "zustand";
import { subscribeWithSelector } from "zustand/middleware";
import type { Prompt, Folder, Tag } from "@/bindings";
import { commands, events } from "@/bindings";

interface PromptLibraryStore {
  prompts: Prompt[];
  folders: Folder[];
  tags: Tag[];
  isLoading: boolean;
  search: string;
  selectedFolderId: number | null;
  selectedTag: string | null;
  sort: string;

  setSearch: (s: string) => void;
  setSelectedFolderId: (id: number | null) => void;
  setSelectedTag: (t: string | null) => void;
  setSort: (s: string) => void;
  refresh: () => Promise<void>;
  refreshFolders: () => Promise<void>;
  refreshTags: () => Promise<void>;
}

export const usePromptLibraryStore = create<PromptLibraryStore>()(
  subscribeWithSelector((set, get) => ({
    prompts: [],
    folders: [],
    tags: [],
    isLoading: true,
    search: "",
    selectedFolderId: null,
    selectedTag: null,
    sort: "updatedDesc",

    setSearch: (search) => set({ search }),
    setSelectedFolderId: (selectedFolderId) => set({ selectedFolderId }),
    setSelectedTag: (selectedTag) => set({ selectedTag }),
    setSort: (sort) => set({ sort }),

    refresh: async () => {
      const { search, selectedFolderId, selectedTag, sort } = get();
      const trimmed = search.trim();
      set({ isLoading: true });
      try {
        // When a search query is present, use FTS5 `searchPrompts` (bm25 +
        // pin, LIMIT 50, spawn_blocking on the Rust side). It handles folder
        // filtering in SQL; tag filtering is applied client-side when both
        // search and tag are active so the FTS path stays fast. Empty query
        // falls through to the no-FTS `listPrompts` path.
        if (trimmed) {
          const res = await commands.searchPrompts(trimmed, selectedFolderId, 50);
          if (res.status === "ok") {
            let prompts: Prompt[] = res.data;
            if (selectedTag) {
              prompts = prompts.filter((p: Prompt) => p.tags.includes(selectedTag));
            } else if (sort && sort !== "updatedDesc") {
              // FTS path already orders pinned/rank/updated; re-sort only
              // when caller asked for a non-default sort and no tag filter
              // hides the FTS ranking.
              // Keep FTS order for search; sort override is deferred to
              // no-search path to preserve bm25 relevance.
            }
            set({ prompts });
          } else {
            // FTS syntax error already falls back to LIKE in Rust, so an
            // error here is unexpected — log and clear.
            console.error("searchPrompts failed", res.error);
          }
        } else {
          const res = await commands.listPrompts({
            search: null,
            folder_id: selectedFolderId,
            tag: selectedTag,
            pinned_only: null,
            sort: sort as any,
            limit: null,
            offset: null,
          });
          if (res.status === "ok") set({ prompts: res.data });
        }
      } catch (e) {
        console.error("Failed to list prompts", e);
      } finally {
        set({ isLoading: false });
      }
    },

    refreshFolders: async () => {
      try {
        const res = await commands.listFolders();
        if (res.status === "ok") set({ folders: res.data });
      } catch (e) {
        console.error("Failed to list folders", e);
      }
    },

    refreshTags: async () => {
      try {
        const res = await commands.listTags();
        if (res.status === "ok") set({ tags: res.data });
      } catch (e) {
        console.error("Failed to list tags", e);
      }
    },
  })),
);

// Subscribe to real-time updates — functional setState avoids stale closure
events.promptLibraryUpdatePayload.listen((event: { payload: any }) => {
  const payload = event.payload;
  if (payload.action === "added") {
    usePromptLibraryStore.setState((state) => ({ prompts: [payload.prompt, ...state.prompts] }));
  } else if (payload.action === "updated" || payload.action === "pinned") {
    const updated = (payload as any).prompt ?? null;
    if (updated) {
      usePromptLibraryStore.setState((state) => ({
        prompts: state.prompts.map((p) => (p.id === updated.id ? updated : p)),
      }));
    } else {
      usePromptLibraryStore.getState().refresh();
    }
  } else if (payload.action === "deleted") {
    usePromptLibraryStore.setState((state) => ({
      prompts: state.prompts.filter((p) => p.id !== payload.id),
    }));
  }
}).catch(() => {});
