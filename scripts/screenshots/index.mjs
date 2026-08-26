export * from "./cli.mjs";
export * from "./mock.mjs";
export * from "./vite.mjs";
export * from "./browser.mjs";
export * from "./config.mjs";
export * from "./setups.mjs";
export * from "./capture.mjs";

import { startVite } from "./vite.mjs";
import { captureAll } from "./capture.mjs";
import { DEFAULT_PORT } from "./cli.mjs";

export async function run(options = {}) {
  let viteServer = null;
  let baseUrl = options.externalUrl ?? null;

  if (!baseUrl) {
    const desiredPort = options.desiredPort ?? options.port ?? DEFAULT_PORT;
    const usePreview = options.usePreview ?? false;
    const started = await startVite(desiredPort, usePreview);
    viteServer = started.server;
    baseUrl = started.url;
  } else {
    console.log(`[capture] Using external URL: ${baseUrl}`);
  }

  let results = [];
  try {
    results = await captureAll(baseUrl, options);
  } finally {
    if (viteServer) {
      await viteServer.close();
      console.log("[capture] Vite server closed");
    }
  }

  if (results.length === 0) {
    console.error("[capture] No screenshots were generated");
    process.exit(1);
  }
  return results;
}
