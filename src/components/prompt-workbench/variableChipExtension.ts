import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import type { Node } from "@tiptap/pm/model";

const VAR_RE = /\{\{\s*([A-Za-z0-9_]+)\s*\}\}/g;

export const VariableChipExtension = Extension.create({
  name: "variableChip",
  addProseMirrorPlugins() {
    const key = new PluginKey("variableChip");

    const build = (doc: Node): DecorationSet => {
      const text = doc.textContent;
      if (!text.includes("{{")) return DecorationSet.empty;
      const decorations: Decoration[] = [];

      // Need to map string offsets to doc positions. Reuse walk similar to spellCheckExtension.
      const positions = positionsForDoc(doc, text);
      if (!positions) return DecorationSet.empty;

      let m: RegExpExecArray | null;
      VAR_RE.lastIndex = 0;
      while ((m = VAR_RE.exec(text)) !== null) {
        const start = m.index;
        const end = start + m[0].length;
        const from = positions[start];
        const to = positions[end];
        if (from === undefined || to === undefined || from === to) continue;
        // Skip if spans block boundary
        try {
          if (doc.resolve(from).parent !== doc.resolve(to).parent) continue;
        } catch {
          continue;
        }
        decorations.push(
          Decoration.inline(from, to, {
            class: "prompt-var-chip",
            "data-var": m[1],
          }),
        );
      }
      return DecorationSet.create(doc, decorations);
    };

    return [
      new Plugin({
        key,
        state: {
          init: (_c, state) => build(state.doc),
          apply: (tr, old) => (tr.docChanged ? build(tr.doc) : old),
        },
        props: {
          decorations: (state) => key.getState(state) as DecorationSet,
        },
      }),
    ];
  },
});

function positionsForDoc(doc: Node, text: string): number[] | null {
  const positions = new Array<number>(text.length + 1);
  let acc = 0;
  let lastEnd = -1;
  const walk = (node: Node, startPos: number): void => {
    if (node.isText) {
      const t = node.text ?? "";
      for (let i = 0; i < t.length; i++) positions[acc + i] = startPos + i;
      acc += t.length;
      lastEnd = startPos + t.length;
      return;
    }
    node.forEach((child, offset) => walk(child, startPos + offset + 1));
  };
  doc.forEach((child, offset) => walk(child, offset + 1));
  if (text.length > 0) positions[text.length] = lastEnd;
  return positions;
}
