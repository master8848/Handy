#!/usr/bin/env node
import { parseArgs } from "./screenshots/cli.mjs";
import { startVite } from "./screenshots/vite.mjs";
import { captureAll } from "./screenshots/capture.mjs";

const opts = parseArgs();

async function main() {
  let viteServer = null;
  let baseUrl = opts.externalUrl;
  if (!baseUrl) {
    const started = await startVite(opts.desiredPort, opts.usePreview);
    viteServer = started.server;
    baseUrl = started.url;
  } else {
    console.log(`[capture] Using external URL: ${baseUrl}`);
  }
  let results = [];
  try {
    results = await captureAll(baseUrl, opts);
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
}

main().catch((e) => {
  console.error("[capture] Fatal:", e);
  process.exit(1);
});
