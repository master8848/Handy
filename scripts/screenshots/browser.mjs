export async function resolveChromium() {
  try {
    const pw = await import("playwright");
    if (pw.chromium) return pw.chromium;
  } catch {}
  try {
    const pwTest = await import("@playwright/test");
    if (pwTest.chromium) return pwTest.chromium;
  } catch {}
  try {
    const pwc = await import("playwright-core");
    if (pwc.chromium) return pwc.chromium;
  } catch {}
  throw new Error(
    "Could not resolve a Playwright chromium export. Run: bunx playwright install chromium ( @playwright/test should already be installed )"
  );
}
