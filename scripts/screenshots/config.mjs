export function defineShot(o) {
  return o;
}

export const DESCRIPTIONS = {
  "dictate-desktop.png": "Dictation persona — desktop",
  "prompt-desktop.png": "Prompt Studio — desktop",
  "transcribe-desktop.png": "File transcription — desktop",
  "toolbar-dropdown.png": "Toolbar dropdown open (experimental)",
  "prompt-workbench-expanded.png": "Prompt Workbench expanded with sample content",
  "dictation-mobile.png": "Dictation — mobile 390px",
  "prompt-mobile.png": "Prompt Studio — mobile 390px",
  "transcribe-mobile.png": "File transcription — mobile 390px",
  "quick-prompt-desktop.png": "Quick Prompt spotlight (Cmd+Shift+J) — desktop 800×600",
  "settings-general-desktop.png": "Settings → General (Quick Prompt & Prompt Palette bindings) — desktop",
};

export const SHOTS = [
  defineShot({
    file: "dictate-desktop.png",
    url: (baseUrl) => `${baseUrl}/?tab=dictate`,
    viewport: { width: 1280, height: 800 },
    wait: '[role="tablist"]',
    setup: null,
    description: DESCRIPTIONS["dictate-desktop.png"],
    category: "desktop",
  }),
  defineShot({
    file: "prompt-desktop.png",
    url: (baseUrl) => `${baseUrl}/?tab=prompt`,
    viewport: { width: 1280, height: 800 },
    wait: '[role="tablist"]',
    setup: null,
    description: DESCRIPTIONS["prompt-desktop.png"],
    category: "desktop",
  }),
  defineShot({
    file: "transcribe-desktop.png",
    url: (baseUrl) => `${baseUrl}/?tab=transcribe`,
    viewport: { width: 1280, height: 800 },
    wait: '[role="tablist"]',
    setup: null,
    description: DESCRIPTIONS["transcribe-desktop.png"],
    category: "desktop",
  }),
  defineShot({
    file: "toolbar-dropdown.png",
    url: (baseUrl) => `${baseUrl}/?tab=dictate`,
    viewport: { width: 1280, height: 800 },
    wait: '[role="tablist"]',
    setup: "toolbarDropdown",
    description: DESCRIPTIONS["toolbar-dropdown.png"],
    category: "overlay",
  }),
  defineShot({
    file: "prompt-workbench-expanded.png",
    url: (baseUrl) => `${baseUrl}/?tab=prompt`,
    viewport: { width: 1280, height: 800 },
    wait: ".ptap-body",
    setup: "promptExpanded",
    description: DESCRIPTIONS["prompt-workbench-expanded.png"],
    category: "desktop",
  }),
  defineShot({
    file: "dictation-mobile.png",
    url: (baseUrl) => `${baseUrl}/?tab=dictate`,
    viewport: { width: 390, height: 844 },
    wait: '[role="tablist"]',
    setup: null,
    description: DESCRIPTIONS["dictation-mobile.png"],
    category: "mobile",
  }),
  defineShot({
    file: "prompt-mobile.png",
    url: (baseUrl) => `${baseUrl}/?tab=prompt`,
    viewport: { width: 390, height: 844 },
    wait: ".ptap-body",
    setup: null,
    description: DESCRIPTIONS["prompt-mobile.png"],
    category: "mobile",
  }),
  defineShot({
    file: "transcribe-mobile.png",
    url: (baseUrl) => `${baseUrl}/?tab=transcribe`,
    viewport: { width: 390, height: 844 },
    wait: '[role="tablist"]',
    setup: null,
    description: DESCRIPTIONS["transcribe-mobile.png"],
    category: "mobile",
  }),
  defineShot({
    file: "quick-prompt-desktop.png",
    url: (baseUrl) => `${baseUrl}/?view=quick-prompt`,
    viewport: { width: 800, height: 600 },
    wait: "textarea",
    setup: "quickPrompt",
    description: DESCRIPTIONS["quick-prompt-desktop.png"],
    category: "overlay",
  }),
  defineShot({
    file: "settings-general-desktop.png",
    url: (baseUrl) => `${baseUrl}/?view=settings`,
    viewport: { width: 1280, height: 800 },
    wait: '[role="tablist"], nav',
    setup: null,
    description: DESCRIPTIONS["settings-general-desktop.png"],
    category: "desktop",
  }),
];

export function getShots(baseUrl) {
  return SHOTS.map((s) => ({
    ...s,
    url: typeof s.url === "function" ? s.url(baseUrl) : s.url,
  }));
}

export function getDescription(file) {
  return DESCRIPTIONS[file] || file;
}
