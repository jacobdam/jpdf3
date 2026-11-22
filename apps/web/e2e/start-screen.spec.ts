import type { Locator } from "@playwright/test";
import { CSP } from "../csp";
import { expect, test } from "./fixtures";

// specs/ui-web/app-skeleton.md §Tests. The W-02 start screen of the production build. Every
// test here gets the shared checks of ./fixtures.ts after its body: no console errors or
// warnings, uncaught errors, CSP violations or foreign requests.

/** Waits until every CSS transition/animation on the element has finished or been cancelled. */
async function settled(locator: Locator): Promise<void> {
  await locator.evaluate((el) =>
    Promise.all(el.getAnimations().map((a) => a.finished.catch(() => undefined))).then(
      () => undefined,
    ),
  );
}

test("start screen loads with the empty state", { tag: "@e2e.web.app_start_empty" }, async ({
  page,
}) => {
  await page.goto("./");

  await expect(page.getByRole("heading", { level: 1, name: "jpdf" })).toBeVisible();
  await expect(page.getByText("Drop a PDF here to open it", { exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { level: 2, name: "Recent files" })).toBeVisible();
  await expect(page.getByText("Files you open appear here.", { exact: true })).toBeVisible();
  await expect(page.getByRole("main")).toBeVisible();
  // W-02 order: heading, drop target text, Open file, Recent files, its empty text.
  const order = await page.getByRole("main").evaluate((main) => {
    const want = [
      "jpdf",
      "Drop a PDF here to open it",
      "Open file",
      "Recent files",
      "Files you open appear here.",
    ];
    const text = main.textContent ?? "";
    return want.map((w) => text.indexOf(w));
  });
  expect(order.every((at, i) => at >= 0 && (i === 0 || at > (order[i - 1] ?? -1)))).toBe(true);

  // Disabled, but focusable, with its reason as the accessible description.
  const open = page.getByRole("button", { name: "Open file" });
  await expect(open).toHaveAttribute("aria-disabled", "true");
  await page.keyboard.press("Tab");
  await expect(open).toBeFocused();
  await expect(open).toHaveAccessibleDescription("Opening files is not available yet");
  // The two-tone focus ring (wireframes-web.md §1.3).
  await expect(open).toHaveCSS("outline", "2px solid rgb(11, 11, 11)");
  await expect(open).toHaveCSS("box-shadow", "rgb(255, 255, 255) 0px 0px 0px 4px");
  // Hovering the focused button keeps both halves of the ring and the resting color.
  await open.hover();
  await settled(open);
  await expect(open).toHaveCSS("outline", "2px solid rgb(11, 11, 11)");
  await expect(open).toHaveCSS("box-shadow", "rgb(255, 255, 255) 0px 0px 0px 4px");
  await expect(open).toHaveCSS("background-color", "rgb(21, 101, 192)");
  // Activating it does nothing: no navigation, no file chooser, no dialog.
  let chooser = false;
  page.on("filechooser", () => {
    chooser = true;
  });
  await page.keyboard.press("Enter");
  await open.click({ force: true });
  await expect(page).toHaveURL("http://127.0.0.1:4173/");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(chooser).toBe(false);

  // Emotion's styles applied under the CSP: the theme's primary color (light appearance).
  // The pointer is still over the button after the click above: a disabled control keeps its
  // resting color on hover (no hover darkening).
  await settled(open);
  await expect(open).toHaveCSS("background-color", "rgb(21, 101, 192)");
  await expect(open).toHaveCSS("text-transform", "none");
  await expect(page.getByText("Files you open appear here.", { exact: true })).toHaveCSS(
    "color",
    "rgb(95, 95, 95)",
  );
});

test("dark appearance uses the dark palette", { tag: "@e2e.web.app_start_empty" }, async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("./");
  const open = page.getByRole("button", { name: "Open file" });
  await expect(open).toHaveCSS("background-color", "rgb(144, 202, 249)");
  await expect(open).toHaveCSS("box-shadow", "none");
  // Unfocused hover: no darkening and no lift (a disabled control looks the same).
  await open.hover();
  await settled(open);
  await expect(open).toHaveCSS("background-color", "rgb(144, 202, 249)");
  await expect(open).toHaveCSS("box-shadow", "none");
  await expect(page.locator("body")).toHaveCSS("background-color", "rgb(18, 18, 18)");
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

test("reduced motion removes transitions", { tag: "@e2e.web.app_start_empty" }, async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("./");
  const open = page.getByRole("button", { name: "Open file" });
  await expect(open).toBeVisible();
  const durations = await open.evaluate((el) => getComputedStyle(el).transitionDuration);
  expect(durations.split(",").every((d) => d.trim() === "0s")).toBe(true);
});

test("the CSP blocks inline script and inline style", {
  tag: "@e2e.web.app_start_empty",
}, async ({ page, watch }) => {
  // Only the violations this test provokes: CSP / Trusted Types console errors and the
  // matching violation events. No uncaught errors, warnings or foreign requests.
  watch.allow(
    (p) =>
      (p.startsWith("console error: ") &&
        /Content Security Policy|TrustedScript|TrustedHTML|Trusted Type/.test(p)) ||
      /^csp violation: (script-src-elem|style-src-elem|require-trusted-types-for) /.test(p),
  );
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
