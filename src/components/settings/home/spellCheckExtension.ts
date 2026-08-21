import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import type { Node } from "@tiptap/pm/model";
import type { SpellingIssue } from "@/bindings";

// Transaction meta flag set by the frontend after a fresh Harper result lands;
// the plugin rebuilds its decorations when it sees it (or any doc change).
export const SPELL_CHECK_META = "spellCheckUpdated";

// Shared cache between the debounced checker (Home) and the editor plugin.
// Keyed by UTF-16 offsets into `doc.textContent`, the same string Harper runs on.
export const spellIssueCache: { issues: SpellingIssue[] } = { issues: [] };

/**
 * Map UTF-16 text offsets to document positions. `text` must be exactly
 * `doc.textContent` (no block separators). `positions[i]` is the doc position
 * where character `i` starts; `positions[text.length]` is the position right
 * after the final character. Offsets at a block boundary (empty paragraph,
 * etc.) are mapped to the nearest text position.
 */
export const positionsFromOffsets = (
  doc: Node,
  text: string,
): number[] | null => {
  const positions = new Array<number>(text.length + 1);
  let acc = 0;
  let lastTextEnd = -1;

  const walk = (node: Node, startPos: number): void => {
    if (node.isText) {
      const nodeText = node.text ?? "";
      for (let i = 0; i < nodeText.length; i++) {
        positions[acc + i] = startPos + i;
      }
      acc += nodeText.length;
      lastTextEnd = startPos + nodeText.length;
      return;
    }
    node.forEach((child, offset) => walk(child, startPos + offset + 1));
  };

  doc.forEach((child, offset) => walk(child, offset + 1));

  if (text.length > 0) {
    positions[text.length] = lastTextEnd;
  }
  return positions;
};

/**
 * Tiptap extension that renders Harper spelling/grammar issues as wavy-underline
 * decorations carrying `data-start`/`data-end` attributes, so the frontend can
 * attach hover popovers to the underlined words.
 */
export const SpellCheckExtension = Extension.create({
  name: "spellCheck",

  addProseMirrorPlugins() {
    const key = new PluginKey("spellCheck");

    const buildDecorations = (doc: Node): DecorationSet => {
      const text = doc.textContent;
      const positions = positionsFromOffsets(doc, text);
      if (!positions || spellIssueCache.issues.length === 0) {
        return DecorationSet.empty;
      }
      const decorations: Decoration[] = [];
      for (const issue of spellIssueCache.issues) {
        if (issue.start < 0 || issue.end > text.length) continue;
        const from = positions[issue.start];
        const to = positions[issue.end];
        if (
          from === undefined ||
          to === undefined ||
          from === to ||
          doc.resolve(from).parent !== doc.resolve(to).parent
        ) {
          continue; // spans a block boundary — skip rather than break the doc
        }
        decorations.push(
          Decoration.inline(from, to, {
            class: "tiptap-spell-issue",
            "data-issue": "",
            "data-start": String(issue.start),
            "data-end": String(issue.end),
          }),
        );
      }
      return DecorationSet.create(doc, decorations);
    };

    return [
      new Plugin({
        key,
        state: {
          init: (_config, state) => buildDecorations(state.doc),
          apply: (tr, oldSet) => {
            if (!tr.docChanged && !tr.getMeta(SPELL_CHECK_META)) {
              return oldSet;
            }
            return buildDecorations(tr.doc);
          },
        },
        props: {
          decorations: (state) => key.getState(state) as DecorationSet,
        },
      }),
    ];
  },
});
