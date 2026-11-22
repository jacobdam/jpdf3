import { test as base, expect, type Page } from "@playwright/test";

// The shared browser-test fixture (specs/ui-web/app-skeleton.md §Tests). Every test that uses
// `page` from this module is checked after its body for console errors and warnings, uncaught
// errors, CSP violations and requests outside the app's origin. A test that provokes some of
// these on purpose (the CSP test) passes a filter to `watch.allow`; everything else still
// fails it. Tests that never use `page` (the reporter unit tests) open no browser.

declare global {
  interface Window {
    __cspViolations?: string[];
  }
}

/** The origin of `vite preview` in playwright.config.ts. */
export const ORIGIN = "http://127.0.0.1:4173/";

export type Watch = {
  /** Problems seen so far, each prefixed with its kind (`console error: `, `csp violation: `…). */
  readonly problems: string[];
  /** Problems matching `filter` don't fail the test. One filter per test; the last call wins. */
  allow(filter: (problem: string) => boolean): void;
};

const watches = new WeakMap<Page, Watch & { allowed: (problem: string) => boolean }>();

async function install(page: Page, origin: string): Promise<void> {
  const problems: string[] = [];
  const w = {
    problems,
    allowed: (_: string) => false,
    allow(filter: (problem: string) => boolean) {
      w.allowed = filter;
    },
  };
  watches.set(page, w);
  page.on("console", (m) => {
    if (m.type() === "error" || m.type() === "warning")
      problems.push(`console ${m.type()}: ${m.text()}`);
  });
  page.on("pageerror", (e) => problems.push(`uncaught: ${e.message}`));
  page.on("request", (r) => {
    if (!r.url().startsWith(origin)) problems.push(`foreign request: ${r.url()}`);
  });
  // Bindings and init scripts run through the DevTools protocol, outside the page's CSP. The
  // binding keeps violations of documents the test navigated away from; the in-page list
  // gives the current document's synchronously at the end.
  await page.exposeBinding("__jpdfCspViolation", (_source, v: string) => {
    problems.push(`csp violation: ${v}`);
  });
  await page.addInitScript(() => {
    window.__cspViolations = [];
    document.addEventListener("securitypolicyviolation", (e) => {
      const v = `${e.effectiveDirective} ${e.blockedURI}`;
      window.__cspViolations?.push(v);
      (window as unknown as { __jpdfCspViolation?: (v: string) => void }).__jpdfCspViolation?.(v);
    });
  });
}

async function verify(page: Page): Promise<void> {
  const w = watches.get(page);
  if (w === undefined) throw new Error("fixtures: page was not watched");
  // The binding reports every violation (the probes in harness-probe/ pin that). The current
  // document's list is a backstop in case a binding call is still in flight at the end; no
  // probe can make that happen on purpose, so this read is not pinned by a test.
  const current = page.isClosed() ? [] : await page.evaluate(() => window.__cspViolations ?? []);
  const all = [...w.problems, ...current.map((v) => `csp violation: ${v}`)];
  expect([...new Set(all)].filter((p) => !w.allowed(p))).toEqual([]);
}

export const test = base.extend<{ origin: string; watch: Watch }>({
  origin: [ORIGIN, { option: true }],
  page: async ({ page, origin }, use) => {
    await install(page, origin);
    await use(page);
    await verify(page);
  },
  watch: async ({ page }, use) => {
    const w = watches.get(page);
    if (w === undefined) throw new Error("fixtures: page was not watched");
    await use(w);
  },
});

export { expect };
