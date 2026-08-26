import { createServer as createViteServer } from "vite";
import fs from "node:fs";
import path from "node:path";
import net from "node:net";
import { REPO_ROOT } from "./cli.mjs";

export function ensureDir(dir) {
  fs.mkdirSync(dir, { recursive: true });
}

export async function isPortFree(port) {
  return new Promise((resolve) => {
    const srv = net.createServer();
    srv.once("error", () => resolve(false));
    srv.once("listening", () => srv.close(() => resolve(true)));
    srv.listen(port, "127.0.0.1");
  });
}

export async function findFreePort(start) {
  for (let p = start; p < start + 20; p++) {
    if (await isPortFree(p)) return p;
  }
  return start;
}

export async function startVite(port, usePreview = false) {
  if (usePreview) {
    console.warn("[capture] --preview requested but dev server is used (build not needed for screenshots)");
  }
  const vitePort = await findFreePort(port);
  console.log(`[capture] Starting Vite dev server on http://127.0.0.1:${vitePort} ...`);
  const server = await createViteServer({
    configFile: path.join(REPO_ROOT, "vite.config.ts"),
    server: { port: vitePort, strictPort: true, host: "127.0.0.1" },
    logLevel: "warn",
  });
  await server.listen();
  server.printUrls();
  const url = `http://127.0.0.1:${vitePort}`;
  for (let i = 0; i < 30; i++) {
    try {
      const res = await fetch(url);
      if (res.ok) break;
    } catch {}
    await new Promise((r) => setTimeout(r, 300));
  }
  return { server, url, port: vitePort };
}
