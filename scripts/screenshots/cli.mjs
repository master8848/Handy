import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
export const REPO_ROOT = path.resolve(__dirname, "../..");
export const SCREENSHOTS_DIR = path.join(REPO_ROOT, "screenshots");
export const PUBLIC_SCREENSHOTS_DIR = path.join(REPO_ROOT, "public", "screenshots");
export const DEFAULT_PORT = 5173;

export const HELP = `
capture-screenshots — capture Handy persona UIs with Playwright

Usage:
  node scripts/capture-screenshots.mjs [options]
  bun run screenshots:capture
  bun run screenshots:capture -- --port 5173 --headed

Options:
  --help, -h           Show this help
  --port <n>           Vite port (default: 5173, auto-bumps if busy)
  --preview            Build then serve preview instead of dev server
  --headed             Run browser headed (for debugging)
  --no-build-mirror    Skip mirroring screenshots/ -> public/screenshots/
  --url <url>          Skip starting Vite, capture from this URL instead
  --only <name>        Only capture one file (e.g. dictate-desktop.png)

Add a new screenshot:
  1. Add entry to scripts/screenshots/config.mjs  (defineShot({...}))
  2. If it needs DOM setup, add handler in scripts/screenshots/setups.mjs
  3. Run: bun run screenshots:capture -- --only your-file.png

Output:
  screenshots/*.png  (mirrored to public/screenshots/ for /screenshots/* dev serving)
  Markdown table printed to stdout
`;

export function parseArgs(argv = process.argv.slice(2)) {
  if (argv.includes("--help") || argv.includes("-h")) {
    console.log(HELP);
    process.exit(0);
  }
  const argVal = (name, fallback) => {
    const i = argv.indexOf(name);
    if (i !== -1 && argv[i + 1]) return argv[i + 1];
    return fallback;
  };
  return {
    headed: argv.includes("--headed"),
    usePreview: argv.includes("--preview"),
    noMirror: argv.includes("--no-build-mirror"),
    onlyFile: argVal("--only", null),
    externalUrl: argVal("--url", null),
    desiredPort: Number(argVal("--port", String(DEFAULT_PORT))) || DEFAULT_PORT,
    argv,
  };
}
