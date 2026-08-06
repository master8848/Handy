# Third-party notices

PromptClear is built on the following third-party works. Their licenses are
reproduced below; see each project for the full license text.

## handy_core — MIT

The `handy_core` crate is extracted from [Handy](https://github.com/cjpais/Handy),
Copyright (c) 2025 CJ Pais, licensed under the MIT License. Full text:
[handy_core/LICENSE](handy_core/LICENSE).

> What it is: offline audio capture (cpal), Silero VAD, audio resampling,
> model management with resumable downloads, and Whisper-family/ONNX
> transcription — the engine behind PromptClear's dictation.

## egui_spellcheck — MPL-2.0

egui_spellcheck (https://github.com/STocidlowski/egui_spellcheck) provides
spell-checking and grammar-checking widgets for egui. It is licensed under the
Mozilla Public License 2.0. Full text: https://www.mozilla.org/MPL/2.0/

> What it is: the red underlines and grammar-suggestion UI in the editor, plus
> the "add word" personal dictionary.

## egui-shadcn — MIT

egui-shadcn (https://github.com/pjankiewicz/egui-shadcn), Copyright (c) Pavel
Jankiewicz, is licensed under the MIT License. Full text:
[egui-shadcn/LICENSE](egui-shadcn/LICENSE).

> What it is: the shadcn/ui-styled widget set (buttons, selects, cards,
> tabs, status bar, switches, sliders, …) that the whole PromptClear UI is
> built on. Vendored at `promptclear/egui-shadcn` and migrated from egui 0.33
> to egui 0.35 (with egui_flex bumped 0.5 → 0.7); the vendored copy's
> examples, web demo, and icon-generation scripts are removed.

## harper-core — Apache-2.0

harper-core (https://github.com/automattic/harper) is the grammar-checking
engine. It is licensed under the Apache License 2.0. Full text:
https://www.apache.org/licenses/LICENSE-2.0

> What it is: live English grammar rules (with a curated English dictionary)
> that produce the grammar suggestions shown in the editor.

## spellbook — MPL-2.0

spellbook (https://github.com/helix-editor/spellbook) is the Hunspell-format
spellchecking library. It is licensed under the Mozilla Public License 2.0.
Full text: https://www.mozilla.org/MPL/2.0/

> What it is: the spellchecking engine behind the bundled en-US dictionary.

## Bundled en-US dictionary — GPL-2.0 (separate from the crates)

egui_spellcheck embeds an en-US Hunspell dictionary (~50k base entries, word
list based on WordNet 2.1/SCOWL) from
https://github.com/JetBrains/hunspell-dictionaries. These data files are
licensed **separately** from the crates (GPL-2.0 with the WordNet exception;
see the `en_US_license.txt`/`en_US_WordNet_license.txt` files in the
egui_spellcheck source).

---

Handy itself (https://github.com/cjpais/Handy) is MIT licensed, Copyright (c)
2025 CJ Pais; this project is not affiliated with it.
