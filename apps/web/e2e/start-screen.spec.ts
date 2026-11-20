import { expect, type Page, test } from "@playwright/test";
import { CSP } from "../csp";

// specs/ui-web/app-skeleton.md §Tests. The W-02 start screen of the production build.

declare global {
  interface Window {
    __cspViolations?: string[];
  }
}

/** Collects console errors/warnings, uncaught exceptions, CSP violations and foreign requests. */
async function watch(page: Page): Promise<string[]> {
  const problems: string[] = [];
  page.on("console", (m) => {
    if (m.type() === "error" || m.type() === "warning")
      problems.push(`console ${m.type()}: ${m.text()}`);
  });
  page.on("pageerror", (e) => problems.push(`uncaught: ${e.message}`));
  page.on("request", (r) => {
    if (!r.url().startsWith("http://127.0.0.1:4173/")) problems.push(`foreign request: ${r.url()}`);
  });
  // Init scripts run through the DevTools protocol, outside the page's CSP.
  await page.addInitScript(() => {
    window.__cspViolations = [];
    document.addEventListener("securitypolicyviolation", (e) => {
      window.__cspViolations?.push(`${e.effectiveDirective} ${e.blockedURI}`);
    });
  });
  return problems;
}

test("start screen loads with the empty state", { tag: "@e2e.web.app_start_empty" }, async ({
  page,
}) => {
  const problems = await watch(page);
  await page.goto("./");

  await expect(page.getByRole("heading", { level: 1, name: "jpdf" })).toBeVisible();
  await expect(page.getByText("Drop a PDF here to open it", { exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { level: 2, name: "Recent files" })).toBeVisible();
  await expect(page.getByText("Files you open appear here.", { exact: true })).toBeVisible();
  await expect(page.getByRole("main")).toBeVisible();

  // Disabled, but focusable, with its reason as the accessible description.
  const open = page.getByRole("button", { name: "Open file" });
  await expect(open).toHaveAttribute("aria-disabled", "true");
  await page.keyboard.press("Tab");
  await expect(open).toBeFocused();
  await expect(open).toHaveAccessibleDescription("Opening files is not available yet");

  // Emotion's styles applied under the CSP: the theme's primary color (light appearance).
  await expect(open).toHaveCSS("background-color", "rgb(21, 101, 192)");
  await expect(open).toHaveCSS("text-transform", "none");
  await expect(page.getByText("Files you open appear here.", { exact: true })).toHaveCSS(
    "color",
    "rgb(95, 95, 95)",
  );

  expect(await page.evaluate(() => window.__cspViolations)).toEqual([]);
  expect(problems).toEqual([]);
});

test("dark appearance uses the dark palette", { tag: "@e2e.web.app_start_empty" }, async ({
  page,
}) => {
  const problems = await watch(page);
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("./");
  await expect(page.getByRole("button", { name: "Open file" })).toHaveCSS(
    "background-color",
    "rgb(144, 202, 249)",
  );
  await expect(page.locator("body")).toHaveCSS("background-color", "rgb(18, 18, 18)");
  expect(await page.evaluate(() => window.__cspViolations)).toEqual([]);
  expect(problems).toEqual([]);
});

test("the CSP meta tag comes first and scripts are external", {
  tag: "@e2e.web.app_start_empty",
}, async ({ page }) => {
  await page.goto("./");
  const head = await page.evaluate(() => {
    const first = document.head.firstElementChild;
    return {
      firstEquiv: first?.getAttribute("http-equiv") ?? null,
      firstContent: first?.getAttribute("content") ?? null,
      metaCount: document.querySelectorAll('meta[http-equiv="Content-Security-Policy"]').length,
      inlineScripts: [...document.scripts].filter((s) => s.src === "").length,
    };
  });
  expect(head).toEqual({
    firstEquiv: "Content-Security-Policy",
    firstContent: CSP,
    metaCount: 1,
    inlineScripts: 0,
  });
});

test("the CSP blocks inline script and inline style", {
  tag: "@e2e.web.app_start_empty",
}, async ({ page }) => {
  await watch(page);
  await page.goto("./");
  await expect(page.getByRole("heading", { level: 1, name: "jpdf" })).toBeVisible();
  const result = await page.evaluate(() => {
    const s = document.createElement("script");
    try {
      // Trusted Types may already refuse the text; either way the script must not run.
      s.append(document.createTextNode("window.__inlineRan = true"));
      document.body.append(s);
    } catch {
      // blocked before insertion
    }
    const st = document.createElement("style");
    st.textContent = "body { outline: 7px solid red }";
    document.head.append(st);
    return {
      inlineRan: (window as unknown as { __inlineRan?: boolean }).__inlineRan === true,
      outline: getComputedStyle(document.body).outlineWidth,
    };
  });
  expect(result.inlineRan).toBe(false);
  expect(result.outline).not.toBe("7px");
  // Trusted Types: HTML-string sinks (insertAdjacentHTML, innerHTML, …) throw.
  const innerHtmlBlocked = await page.evaluate(() => {
    try {
      document.createElement("div").insertAdjacentHTML("beforeend", "<b>x</b>");
      return false;
    } catch {
      return true;
    }
  });
  expect(innerHtmlBlocked).toBe(true);
});

test("a file dropped on the window is not opened by the browser", {
  tag: "@e2e.web.app_start_empty",
}, async ({ page }) => {
  await page.goto("./");
  await expect(page.getByRole("heading", { level: 1, name: "jpdf" })).toBeVisible();
  const prevented = await page.evaluate(() => {
    const dt = new DataTransfer();
    dt.items.add(new File(["%PDF-1.7\n"], "a.pdf", { type: "application/pdf" }));
    return (["dragover", "drop"] as const).map((type) => {
      const ev = new DragEvent(type, { dataTransfer: dt, bubbles: true, cancelable: true });
      document.body.dispatchEvent(ev);
      return ev.defaultPrevented;
    });
  });
  expect(prevented).toEqual([true, true]);
  await expect(page).toHaveURL("http://127.0.0.1:4173/");
});
