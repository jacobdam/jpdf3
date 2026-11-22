import type { Page } from "@playwright/test";

// Pages for the fixture probes, served by routing (no server). PROBE_ORIGIN is the `origin`
// option in ./playwright.config.ts, so these requests are not foreign.
export const PROBE_ORIGIN = "http://probe.test/";

const CLEAN = "<!doctype html><title>clean</title><p>clean</p>";
// An inline script under `script-src 'none'`: a CSP violation (and a console error).
const VIOLATES = `<!doctype html><meta http-equiv="Content-Security-Policy" content="script-src 'none'"><title>v</title><script>window.ran = 1</script>`;

export async function serve(page: Page): Promise<void> {
  await page.route(`${PROBE_ORIGIN}**`, (route) => {
    const body = route.request().url().endsWith("/violates") ? VIOLATES : CLEAN;
    return route.fulfill({ contentType: "text/html", body });
  });
}
