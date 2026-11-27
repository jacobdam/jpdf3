import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { CSP } from "../csp";
import { expect, test } from "./fixtures";

// specs/tooling/dashboard.md §Tests. The metrics dashboard page of the production build, with
// its snapshot (./data.json) served by page.route. No journey ID: the dashboard is project
// tooling, and a failing test here still fails the run (and so `cargo xtask ci linux`).

const MONDAYS = [
  "2026-09-07",
  "2026-09-14",
  "2026-09-21",
  "2026-09-28",
  "2026-10-05",
  "2026-10-12",
  "2026-10-19",
  "2026-10-26",
  "2026-11-02",
  "2026-11-09",
  "2026-11-16",
  "2026-11-23",
];

function sunday(monday: string): string {
  const d = new Date(`${monday}T00:00:00Z`);
  d.setUTCDate(d.getUTCDate() + 6);
  return d.toISOString().slice(0, 10);
}

function snapshot(over: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    version: 1,
    generated: "2026-11-25",
    until: "2026-11-25",
    weeks: MONDAYS.map((m, i) => ({
      start: m,
      end: i === 11 ? "2026-11-25" : sunday(m),
      merged_prs: i === 11 ? 49 : i === 10 ? 101 : 0,
      median_rounds: i === 10 ? 2 : i === 11 ? 3 : null,
      sessions: i === 11 ? 11 : 0,
    })),
    sessions: "index",
    journal_entries: {
      prs: 35,
      median: 1,
      max: 4,
      histogram: [
        { entries: 1, prs: 28 },
        { entries: 2, prs: 5 },
        { entries: 3, prs: 1 },
        { entries: 4, prs: 1 },
      ],
    },
    blocked: [{ number: 231, title: "<b>not markup</b>", age_days: 3 }],
    milestones: [
      { title: "M0", open: 20, closed: 82, closed_28d: 82, forecast: "2026-12-02" },
      { title: "M1", open: 11, closed: 0, closed_28d: 0, forecast: null },
      { title: "M2", open: 0, closed: 4, closed_28d: 1, forecast: null },
    ],
    truncated: [],
    malformed_index_rows: 0,
    ...over,
  };
}

/** The snapshot with week 0's fields replaced by `over`. */
function withWeek(over: Record<string, unknown>): Record<string, unknown> {
  const s = snapshot();
  const weeks = s.weeks as Record<string, unknown>[];
  weeks[0] = { ...weeks[0], ...over };
  return s;
}

const INVALID = "No metrics snapshot: data.json is missing or invalid.";

async function serve(page: Page, body: unknown, status = 200): Promise<void> {
  await page.route("**/dashboard/data.json", (route) =>
    route.fulfill({ status, contentType: "application/json", body: JSON.stringify(body) }),
  );
}

/** The table row whose first cell is `label`, as its cell texts. */
async function row(page: Page, table: string, label: string): Promise<string[]> {
  return page
    .getByRole("table", { name: table })
    .getByRole("row")
    .filter({ has: page.getByRole("cell", { name: label, exact: true }) })
    .first()
    .getByRole("cell")
    .allTextContents();
}

test("renders every section of a full snapshot", async ({ page }) => {
  await serve(page, snapshot({ truncated: ["issues"] }));
  await page.goto("./dashboard/");
  await expect(page.getByRole("heading", { level: 1, name: "jpdf metrics" })).toBeVisible();
  const h2 = await page.getByRole("heading", { level: 2 }).allTextContents();
  expect(h2).toEqual([
    "Merged PRs per week",
    "Median review rounds per week",
    "Sessions per week",
    "Sessions per merged PR",
    "Blocked issues",
    "Milestone forecast",
  ]);
  await expect(
    page.getByText("Snapshot generated 2026-11-25, data up to 2026-11-25."),
  ).toBeVisible();
  await expect(page.getByText("may be cut off: issues.")).toBeVisible();
  // Charts: an accessible name, one bar per non-null value.
  const merged = page.getByRole("img", { name: "Merged PRs per week" });
  await expect(merged).toBeVisible();
  await expect(merged.locator("rect")).toHaveCount(12);
  await expect(
    page.getByRole("img", { name: "Median review rounds per week" }).locator("rect"),
  ).toHaveCount(2);
  // The partial last week is labeled with its end; a full week is not.
  expect(
    await row(page, "Merged PRs per week: values", "Week of 2026-11-23 (to 2026-11-25)"),
  ).toEqual(["Week of 2026-11-23 (to 2026-11-25)", "49"]);
  expect(await row(page, "Merged PRs per week: values", "Week of 2026-11-16")).toEqual([
    "Week of 2026-11-16",
    "101",
  ]);
  // A null median is "missing", never 0.
  expect(await row(page, "Median review rounds per week: values", "Week of 2026-09-07")).toEqual([
    "Week of 2026-09-07",
    "missing",
  ]);
  expect(await row(page, "Sessions per week: values", "Week of 2026-09-07")).toEqual([
    "Week of 2026-09-07",
    "0",
  ]);
  await expect(
    page.getByText("Session journal entries per merged PR: 35 PRs, median 1, max 4."),
  ).toBeVisible();
  expect(await row(page, "Merged PRs by session journal entries: values", "4 or more")).toEqual([
    "4 or more",
    "1",
  ]);
  // Issue titles are text, not markup.
  expect(await row(page, "Blocked issues", "#231")).toEqual(["#231", "<b>not markup</b>", "3"]);
  await expect(page.locator("main b")).toHaveCount(0);
  expect(await row(page, "Milestone forecast", "M0")).toEqual([
    "M0",
    "82",
    "20",
    "82",
    "2026-12-02",
  ]);
  expect(await row(page, "Milestone forecast", "M1")).toEqual([
    "M1",
    "0",
    "11",
    "0",
    "no closures in 4 weeks",
  ]);
  expect(await row(page, "Milestone forecast", "M2")).toEqual(["M2", "4", "0", "1", "done"]);
});

test("the committed snapshot (cargo xtask dashboard's output) is valid", async ({ page }) => {
  // The Rust writer and this page's validator agree on the format (ops/metrics/dashboard.json).
  const committed = JSON.parse(
    readFileSync(new URL("../../../ops/metrics/dashboard.json", import.meta.url), "utf8"),
  ) as unknown;
  await serve(page, committed);
  await page.goto("./dashboard/");
  await expect(page.getByRole("heading", { level: 2, name: "Milestone forecast" })).toBeVisible();
  await expect(page.getByText(INVALID)).toHaveCount(0);
});

test("a snapshot without a session index shows sessions as missing", async ({ page }) => {
  const s = snapshot({ sessions: "missing", blocked: [], malformed_index_rows: 0 });
  for (const w of s.weeks as Record<string, unknown>[]) w.sessions = null;
  await serve(page, s);
  await page.goto("./dashboard/");
  await expect(page.getByText("Missing: the snapshot has no session index.")).toBeVisible();
  await expect(page.getByRole("img", { name: "Sessions per week" })).toHaveCount(0);
  await expect(page.getByText("No blocked issues.")).toBeVisible();
  await expect(page.getByText("may be cut off")).toHaveCount(0);
});

for (const [name, body] of [
  ["version 2", snapshot({ version: 2 })],
  ["a negative count", snapshot({ malformed_index_rows: -1 })],
  ["a fractional count", snapshot({ malformed_index_rows: 1.5 })],
  ["a date that doesn't exist", snapshot({ until: "2026-02-30" })],
  ["a date in another form", snapshot({ generated: "2026-11-25T00:00:00Z" })],
  ["an unknown sessions source", snapshot({ sessions: "guess" })],
  ["a title that isn't text", snapshot({ blocked: [{ number: 1, title: 7, age_days: 0 }] })],
  ["an array", [snapshot()]],
  ["a week with a negative count", withWeek({ merged_prs: -1 })],
  ["a week with a bad end date", withWeek({ end: "x" })],
  ["a week with a null merged count", withWeek({ merged_prs: null })],
  ["eleven weeks", snapshot({ weeks: (snapshot().weeks as unknown[]).slice(1) })],
  [
    "thirteen weeks",
    snapshot({
      weeks: [
        ...(snapshot().weeks as Record<string, unknown>[]),
        { start: "2026-11-30", end: "2026-11-30", merged_prs: 0, median_rounds: null, sessions: 0 },
      ],
    }),
  ],
  [
    "a histogram with three objects",
    snapshot({
      journal_entries: {
        prs: 1,
        median: 1,
        max: 1,
        histogram: [
          { entries: 1, prs: 0 },
          { entries: 2, prs: 0 },
          { entries: 3, prs: 0 },
        ],
      },
    }),
  ],
  [
    "a histogram with a repeated entries value",
    snapshot({
      journal_entries: {
        prs: 1,
        median: 1,
        max: 1,
        histogram: [
          { entries: 1, prs: 0 },
          { entries: 1, prs: 0 },
          { entries: 2, prs: 0 },
          { entries: 3, prs: 0 },
        ],
      },
    }),
  ],
  [
    "two weeks with one start",
    snapshot({
      weeks: (snapshot().weeks as Record<string, unknown>[]).map((w, i) =>
        i === 1 ? { ...w, start: MONDAYS[0] } : w,
      ),
    }),
  ],
  [
    "a histogram entries value past 4",
    snapshot({
      journal_entries: {
        prs: 1,
        median: 1,
        max: 5,
        histogram: [
          { entries: 1, prs: 0 },
          { entries: 2, prs: 0 },
          { entries: 3, prs: 0 },
          { entries: 5, prs: 1 },
        ],
      },
    }),
  ],
  [
    "a histogram count that isn't an integer",
    snapshot({
      journal_entries: {
        prs: 1,
        median: 1,
        max: 1,
        histogram: [
          { entries: 1, prs: 0.5 },
          { entries: 2, prs: 0 },
          { entries: 3, prs: 0 },
          { entries: 4, prs: 0 },
        ],
      },
    }),
  ],
  [
    "a blocked age that is null",
    snapshot({ blocked: [{ number: 1, title: "x", age_days: null }] }),
  ],
  [
    "a forecast that isn't a date",
    snapshot({
      milestones: [{ title: "M0", open: 1, closed: 0, closed_28d: 1, forecast: "soon" }],
    }),
  ],
  [
    "a fractional milestone count",
    snapshot({
      milestones: [{ title: "M0", open: 1.5, closed: 0, closed_28d: 1, forecast: null }],
    }),
  ],
] as const) {
  test(`an invalid snapshot (${name}) shows the invalid state`, async ({ page }) => {
    await serve(page, body);
    await page.goto("./dashboard/");
    await expect(page.getByText(INVALID)).toBeVisible();
    await expect(page.getByRole("heading", { level: 2 })).toHaveCount(0);
  });
}

test("a missing snapshot (404) shows the invalid state", async ({ page, watch }) => {
  // The browser logs the failed request; nothing else may go wrong.
  watch.allow((p) => /^console error: Failed to load resource: .*404/.test(p));
  await serve(page, { error: "not found" }, 404);
  await page.goto("./dashboard/");
  await expect(page.getByText(INVALID)).toBeVisible();
  await expect(page.getByRole("heading", { level: 2 })).toHaveCount(0);
});

test("the dashboard has the CSP meta tag first and no inline script", async ({ page }) => {
  await serve(page, snapshot());
  await page.goto("./dashboard/");
  await expect(page.getByRole("heading", { level: 2, name: "Milestone forecast" })).toBeVisible();
  const head = await page.evaluate(() => ({
    first: document.head.firstElementChild?.getAttribute("content") ?? null,
    inline: [...document.scripts].filter((s) => s.src === "").length,
  }));
  expect(head).toEqual({ first: CSP, inline: 0 });
});

/** WCAG 2 contrast ratio of two computed `rgb(...)` colors. */
function contrast(a: string, b: string): number {
  const lum = (c: string) => {
    const m = /^rgba?\((\d+), (\d+), (\d+)/.exec(c);
    if (m === null) throw new Error(`not an rgb color: ${c}`);
    const [r, g, bl] = [m[1], m[2], m[3]].map((x) => {
      const v = Number(x) / 255;
      return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
    }) as [number, number, number];
    return 0.2126 * r + 0.7152 * g + 0.0722 * bl;
  };
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p) as [number, number];
  return (x + 0.05) / (y + 0.05);
}

for (const scheme of ["light", "dark"] as const) {
  test(`text and bars have enough contrast (${scheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await serve(page, snapshot());
    await page.goto("./dashboard/");
    // A bar with a non-zero value (a 0 bar has no height, so it isn't visible).
    const bar = page
      .getByRole("img", { name: "Merged PRs per week" })
      .locator('rect[data-bar="Week of 2026-11-16"]');
    await expect(bar).toBeVisible();
    const colors = await page.evaluate(() => {
      const css = (sel: string, prop: string) => {
        const el = document.querySelector(sel);
        if (el === null) throw new Error(`missing ${sel}`);
        return getComputedStyle(el).getPropertyValue(prop);
      };
      return {
        bg: css("body", "background-color"),
        h1: css("h1", "color"),
        cell: css("td", "color"),
        secondary: css("main p", "color"),
        bar: css(
          'svg[aria-label="Merged PRs per week"] rect[data-bar="Week of 2026-11-16"]',
          "fill",
        ),
      };
    });
    expect(colors.bg).toBe(scheme === "dark" ? "rgb(18, 18, 18)" : "rgb(245, 245, 245)");
    for (const text of [colors.h1, colors.cell, colors.secondary])
      expect(contrast(text, colors.bg)).toBeGreaterThanOrEqual(4.5);
    expect(contrast(colors.bar, colors.bg)).toBeGreaterThanOrEqual(3);
  });
}
