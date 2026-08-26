import fs from "node:fs";
import path from "node:path";
import { resolveChromium } from "./browser.mjs";
import { TAURI_MOCK_SCRIPT } from "./mock.mjs";
import { ensureDir } from "./vite.mjs";
import { SCREENSHOTS_DIR, PUBLIC_SCREENSHOTS_DIR } from "./cli.mjs";
import { getShots, DESCRIPTIONS } from "./config.mjs";
import { runSetup } from "./setups.mjs";

export async function captureAll(baseUrl, opts = {}) {
  const { headed = false, noMirror = false, onlyFile = null } = opts;
  const chromium = await resolveChromium();
  const browser = await chromium.launch({ headless: !headed });
  const context = await browser.newContext({
    viewport: { width: 1280, height: 800 },
    deviceScaleFactor: 2,
  });
  // inject Tauri mock before any script runs
  await context.addInitScript({ content: TAURI_MOCK_SCRIPT });

  ensureDir(SCREENSHOTS_DIR);
  if (!noMirror) ensureDir(PUBLIC_SCREENSHOTS_DIR);

  const shots = getShots(baseUrl);

  const filtered = onlyFile ? shots.filter((s) => s.file === onlyFile) : shots;
  if (onlyFile && filtered.length === 0) {
    console.error(`[capture] Unknown --only file: ${onlyFile}`);
    process.exit(1);
  }

  const results = [];

  for (const shot of filtered) {
    const page = await context.newPage();
    await page.setViewportSize(shot.viewport);
    console.log(`[capture] ${shot.file} <- ${shot.url} @ ${shot.viewport.width}x${shot.viewport.height}`);
    await page.goto(shot.url, { waitUntil: "domcontentloaded", timeout: 15000 });
    // let React mount
    await page.waitForTimeout(900);
    if (shot.wait) {
      try {
        await page.waitForSelector(shot.wait, { timeout: 5000 });
      } catch {
        console.warn(`[capture] wait selector not found: ${shot.wait} for ${shot.file}`);
      }
    }
    await page.waitForTimeout(400);

    // Prime theme to light for consistent screenshots (mock stays light; dark capture not yet)
    try {
      await page.evaluate(() => {
        document.documentElement.setAttribute("data-theme", "light");
        document.documentElement.classList.remove("dark");
        try {
          localStorage.setItem("handy.theme", "light");
        } catch {}
      });
    } catch {}

    await runSetup(page, shot.setup);

    // Ensure onboarding overlay not covering — if accessibility onboarding is visible, hide it for capture
    await page.evaluate(() => {
      // Force onboarding to done if the mock didn't bypass it
      try {
        const hasOnboarding = document.body.textContent && document.body.textContent.includes("Permissions Required");
        if (hasOnboarding) {
          document.body.innerHTML = '<div style="padding:32px;font-family:system-ui">Onboarding visible — mock incomplete. Check TAURI_MOCK_SCRIPT.</div>';
        }
      } catch {}
    });

    const outPath = path.join(SCREENSHOTS_DIR, shot.file);
    await page.screenshot({ path: outPath, fullPage: true, type: "png" });

    // Optimize: if >2MB, re-encode as compressed png via same file (playwright already compresses)
    try {
      const stat = fs.statSync(outPath);
      if (stat.size > 2 * 1024 * 1024) {
        console.warn(`[capture] ${shot.file} is ${(stat.size / 1024 / 1024).toFixed(2)}MB >2MB — consider recompressing`);
      }
      results.push({ file: shot.file, path: outPath, bytes: stat.size, viewport: shot.viewport });
    } catch {}

    if (!noMirror) {
      try {
        const mirrorPath = path.join(PUBLIC_SCREENSHOTS_DIR, shot.file);
        fs.copyFileSync(outPath, mirrorPath);
      } catch {}
    }

    await page.close();
  }

  await browser.close();

  // Print markdown table
  console.log("\n# Screenshots\n");
  console.log("| Preview | File | Description | Size |");
  console.log("| --- | --- | --- | --- |");
  for (const r of results) {
    const kb = (r.bytes / 1024).toFixed(0);
    const desc = DESCRIPTIONS[r.file] || r.file;
    console.log(`| ![${desc}](screenshots/${r.file}) | \`${r.file}\` | ${desc} | ${kb} KB |`);
  }
  console.log("\nGallery: http://localhost:5173/?view=screenshots  (or `?screenshots=1`)");
  console.log(`Screenshots dir: ${SCREENSHOTS_DIR}`);
  if (!noMirror) console.log(`Mirrored to: ${PUBLIC_SCREENSHOTS_DIR}`);
  return results;
}
