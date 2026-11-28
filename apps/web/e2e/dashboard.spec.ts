import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { CSP } from "../csp";
import { expect, test } from "./fixtures";

// specs/tooling/dashboard.md §Tests. The metrics dashboard page of the production build, with
// its snapshot (./data.json) and the judge report (../judge/report/report.json) served by
// page.route. No journey ID: the dashboard is project
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

/** The judge history: week i has a report (open rate i/20) except weeks 0 to 2 and 6. */
function judge(): Record<string, unknown> {
  return {
    source: "nightly",
    weeks: MONDAYS.map((m, i) =>
      i < 3 || i === 6
        ? {
            start: m,
            run: null,
            day: null,
            open_rate: null,
            render_match: null,
            text_f1: null,
            crashes: null,
            journeys_passing: null,
            journeys: null,
          }
        : {
            start: m,
            run: 45000000000 + i,
            day: sunday(m),
            open_rate: i / 20,
            render_match: 0,
            text_f1: i === 11 ? null : 0.5,
            crashes: i === 11 ? 2 : 0,
            journeys_passing: i,
            journeys: 129,
          },
    ),
  };
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
    judge: judge(),
    truncated: [],
    malformed_index_rows: 0,
    ...over,
  };
}

/** The snapshot with judge week 0's fields replaced by `over`. */
function withJudgeWeek(over: Record<string, unknown>): Record<string, unknown> {
  const j = judge();
  const weeks = j.weeks as Record<string, unknown>[];
  weeks[0] = { ...weeks[0], ...over };
  return snapshot({ judge: j });
}

/** A smoke judge report as verdict.py writes it (only the fields the page reads, plus noise). */
function report(over: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    tier: "smoke",
    pass: true,
    crashes: [],
    failures: [],
    regressions: [],
    metrics: {
      open_rate: 0.5935,
      render_match: 0.0,
      text_f1: null,
      crashes: 1,
      journeys: 6,
      journeys_passing: 3,
    },
    journeys: {
      "J-ANN-01": { milestone: "M5", pass: true, superseded: false },
      "J-ANN-02": { milestone: "M5", pass: false, superseded: false },
      // Superseded: not counted, even though it passes.
      "J-ANN-03": { milestone: "M5", pass: true, superseded: true },
      "J-PC-01": { milestone: "M10", pass: false, superseded: false },
      "J-PC-02": { milestone: "M1", pass: true, superseded: false },
      // Unknown (no macOS results in a push run): counted, not passing.
      "J-PC-03": { milestone: "M1", pass: null, superseded: false },
      "J-X-01": { milestone: "Later", pass: false, superseded: false },
    },
    ...over,
  };
}

/** The report padded with an extra string field to exactly `length` characters. */
function padded(length: number): string {
  const base = JSON.stringify(report());
  const pad = length - base.length - '"pad": "",'.length;
  const body = base.replace("{", `{"pad": "${"x".repeat(pad)}",`);
  if (body.length !== length) throw new Error(`padded: ${body.length} != ${length}`);
  return body;
}

async function serveReport(page: Page, body: unknown, status = 200): Promise<void> {
  await page.route("**/judge/report/report.json", (route) =>
    route.fulfill({
      status,
      contentType: "application/json",
      body: typeof body === "string" ? body : JSON.stringify(body),
    }),
  );
}

// Every page gets a valid report unless a test serves another one (later routes win).
test.beforeEach(async ({ page }) => {
  await serveReport(page, report());
});

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
    "Judge metrics, latest main run",
    "Judge metrics per week",
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
  ["a judge source that is unknown", snapshot({ judge: { ...judge(), source: "guess" } })],
  [
    "eleven judge weeks",
    snapshot({ judge: { ...judge(), weeks: (judge().weeks as unknown[]).slice(1) } }),
  ],
  ["a judge week whose start differs from weeks", withJudgeWeek({ start: "2026-09-08" })],
  ["a judge rate past 1", withJudgeWeek({ open_rate: 1.5 })],
  ["a negative judge rate", withJudgeWeek({ render_match: -0.1 })],
  ["a fractional judge run", withJudgeWeek({ run: 1.5 })],
  ["a negative judge crash count", withJudgeWeek({ crashes: -1 })],
  ["a fractional judge journeys passing count", withJudgeWeek({ journeys_passing: 1.5 })],
  ["a judge journeys count that is text", withJudgeWeek({ journeys: "129" })],
  ["a judge rate that is text", withJudgeWeek({ text_f1: "0.5" })],
  ["a judge day that isn't a date", withJudgeWeek({ day: "today" })],
  ["a judge field that is an array", snapshot({ judge: [] })],
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

test("the judge history shows each week's nightly metrics, missing weeks without a bar", async ({
  page,
}) => {
  await serve(page, snapshot());
  await page.goto("./dashboard/");
  const open = page.getByRole("img", { name: "Smoke open rate per week" });
  await expect(open).toBeVisible();
  await expect(open.locator("rect")).toHaveCount(8);
  for (const name of [
    "Smoke render match per week",
    "Smoke crashes per week",
    "Journeys passing per week",
  ])
    await expect(page.getByRole("img", { name }).locator("rect")).toHaveCount(8);
  await expect(
    page.getByRole("img", { name: "Smoke text F1 per week" }).locator("rect"),
  ).toHaveCount(7);
  const table = "Smoke open rate per week: values";
  expect(await row(page, table, "Week of 2026-11-16")).toEqual(["Week of 2026-11-16", "0.5"]);
  expect(await row(page, table, "Week of 2026-09-07")).toEqual(["Week of 2026-09-07", "missing"]);
  expect(await row(page, table, "Week of 2026-10-19")).toEqual(["Week of 2026-10-19", "missing"]);
  expect(
    await row(page, "Smoke text F1 per week: values", "Week of 2026-11-23 (to 2026-11-25)"),
  ).toEqual(["Week of 2026-11-23 (to 2026-11-25)", "missing"]);
  expect(
    await row(page, "Smoke crashes per week: values", "Week of 2026-11-23 (to 2026-11-25)"),
  ).toEqual(["Week of 2026-11-23 (to 2026-11-25)", "2"]);
  expect(await row(page, "Journeys passing per week: values", "Week of 2026-11-09")).toEqual([
    "Week of 2026-11-09",
    "9",
  ]);
  await expect(page.getByText("Missing: the snapshot has no judge history.")).toHaveCount(0);
});

for (const [name, s] of [
  [
    "no judge field (a snapshot from before #306)",
    (() => {
      const x = snapshot();
      delete x.judge;
      return x;
    })(),
  ],
  [
    "source missing",
    snapshot({
      judge: { ...judge(), source: "missing" },
    }),
  ],
] as const) {
  test(`a snapshot with ${name} shows the judge history as missing`, async ({ page }) => {
    await serve(page, s);
    await page.goto("./dashboard/");
    await expect(page.getByText("Missing: the snapshot has no judge history.")).toBeVisible();
    await expect(page.getByRole("img", { name: "Smoke open rate per week" })).toHaveCount(0);
    // The rest of the page is there.
    await expect(page.getByRole("heading", { level: 2, name: "Milestone forecast" })).toBeVisible();
  });
}

test("the latest judge report shows its smoke metrics and journeys per milestone", async ({
  page,
}) => {
  await serve(page, snapshot());
  await page.goto("./dashboard/");
  const t = "Smoke corpus, latest run";
  await expect(page.getByRole("table", { name: t })).toBeVisible();
  expect(await row(page, t, "Open rate")).toEqual(["Open rate", "0.5935"]);
  expect(await row(page, t, "Render match")).toEqual(["Render match", "0"]);
  expect(await row(page, t, "Text F1")).toEqual(["Text F1", "missing"]);
  expect(await row(page, t, "Crashes")).toEqual(["Crashes", "1"]);
  const rows = await page
    .getByRole("table", { name: "Journeys passing per milestone" })
    .getByRole("row")
    .allTextContents();
  // M1, M5, M10 by number, then other titles; the superseded J-ANN-03 isn't counted.
  expect(rows).toEqual(["MilestonePassingJourneys", "M112", "M512", "M1001", "Later01"]);
});

for (const [name, body, status, message] of [
  ["a 404", { error: "not found" }, 404, "Missing: this site has no judge report."],
  [
    "the exam tier",
    report({ tier: "exam" }),
    200,
    "Invalid: the judge report doesn't have the expected format.",
  ],
  [
    "a rate past 1",
    report({ metrics: { open_rate: 1.5, render_match: 0, text_f1: 0, crashes: 0 } }),
    200,
    "Invalid: the judge report doesn't have the expected format.",
  ],
  [
    "a negative crash count",
    report({ metrics: { open_rate: 1, render_match: 0, text_f1: 0, crashes: -1 } }),
    200,
    "Invalid: the judge report doesn't have the expected format.",
  ],
  [
    "a journey without pass",
    report({ journeys: { "J-1": { milestone: "M1", superseded: false } } }),
    200,
    "Invalid: the judge report doesn't have the expected format.",
  ],
  ["text that isn't JSON", "{", 200, "Invalid: the judge report doesn't have the expected format."],
  [
    "a milestone that isn't text",
    report({ journeys: { "J-1": { milestone: 5, pass: true, superseded: false } } }),
    200,
    "Invalid: the judge report doesn't have the expected format.",
  ],
  [
    "a superseded flag that isn't a boolean",
    report({ journeys: { "J-1": { milestone: "M1", pass: true, superseded: null } } }),
    200,
    "Invalid: the judge report doesn't have the expected format.",
  ],
  [
    "a pass that is text",
    report({ journeys: { "J-1": { milestone: "M1", pass: "yes", superseded: false } } }),
    200,
    "Invalid: the judge report doesn't have the expected format.",
  ],
  [
    "a body of 4 MiB + 1",
    padded(4 * 1024 * 1024 + 1),
    200,
    "Invalid: the judge report doesn't have the expected format.",
  ],
] as const) {
  test(`a judge report (${name}) shows its own state, the rest of the page stays`, async ({
    page,
    watch,
  }) => {
    if (status === 404)
      watch.allow((p) => /^console error: Failed to load resource: .*404/.test(p));
    await serveReport(page, body, status);
    await serve(page, snapshot());
    await page.goto("./dashboard/");
    await expect(page.getByText(message)).toBeVisible();
    await expect(page.getByRole("table", { name: "Smoke corpus, latest run" })).toHaveCount(0);
    await expect(page.getByRole("img", { name: "Smoke open rate per week" })).toBeVisible();
  });
}

test("a judge report of exactly 4 MiB is read", async ({ page }) => {
  await serveReport(page, padded(4 * 1024 * 1024));
  await serve(page, snapshot());
  await page.goto("./dashboard/");
  await expect(page.getByRole("table", { name: "Smoke corpus, latest run" })).toBeVisible();
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
    // An opaque color only: a translucent one would need blending first (#369 R1.1, R1.2).
    const m = /^rgba?\((\d+), (\d+), (\d+)(?:, ([\d.]+))?\)$/.exec(c);
    if (m === null) throw new Error(`not an rgb color: ${c}`);
    if (m[4] !== undefined && Number(m[4]) < 1) throw new Error(`a translucent color: ${c}`);
    const [r, g, bl] = [m[1], m[2], m[3]].map((x) => {
      const v = Number(x) / 255;
      return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
    }) as [number, number, number];
    return 0.2126 * r + 0.7152 * g + 0.0722 * bl;
  };
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p) as [number, number];
  return (x + 0.05) / (y + 0.05);
}

/**
 * Every element of the page matching one of `selectors`, and each of its descendants that
 * holds text, as its selector, text color and effective background (the nearest
 * ancestor-or-self background that isn't transparent).
 */
async function textColors(
  page: Page,
  selectors: readonly string[],
): Promise<{ sel: string; color: string; bg: string }[]> {
  return page.evaluate((sels) => {
    const bgOf = (el: Element | null): string => {
      for (let e = el; e !== null; e = e.parentElement) {
        const style = getComputedStyle(e);
        // A background image has no one color, so `contrast` refuses it, except a gradient
        // whose every stop is fully transparent (MUI's dark-mode Paper overlay at elevation 0).
        const img = style.backgroundImage;
        const clear = /^linear-gradient\((rgba\(\d+, \d+, \d+, 0\)(, )?)+\)$/.test(img);
        if (img !== "none" && !clear) return `image: ${img}`;
        const c = style.backgroundColor;
        if (c !== "transparent" && !/^rgba\(.*, 0\)$/.test(c)) return c;
      }
      return getComputedStyle(document.body).backgroundColor;
    };
    // The matched element and every descendant that holds text of its own (#369 R1.3).
    const texts = (el: Element): Element[] => [
      el,
      ...[...el.querySelectorAll("*")].filter((d) =>
        [...d.childNodes].some((n) => n.nodeType === Node.TEXT_NODE && n.textContent?.trim()),
      ),
    ];
    return sels.flatMap((sel) =>
      [...document.querySelectorAll(sel)].flatMap(texts).map((el) => ({
        sel,
        color: getComputedStyle(el).color,
        bg: bgOf(el),
      })),
    );
  }, selectors);
}

/** `textColors` with at least one element per selector, each at 4.5:1 or more. */
async function expectReadable(page: Page, selectors: readonly string[]): Promise<void> {
  const found = await textColors(page, selectors);
  for (const sel of selectors)
    expect(found.filter((f) => f.sel === sel).length, `${sel} on the page`).toBeGreaterThan(0);
  for (const f of found)
    expect(contrast(f.color, f.bg), `${f.sel}: ${f.color} on ${f.bg}`).toBeGreaterThanOrEqual(4.5);
}

for (const scheme of ["light", "dark"] as const) {
  test(`text and bars have enough contrast (${scheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    // The truncation warning is on, so its Alert is checked too.
    await serve(page, snapshot({ truncated: ["issues"] }));
    await page.goto("./dashboard/");
    // A bar with a non-zero value (a 0 bar has no height, so it isn't visible).
    const bar = page
      .getByRole("img", { name: "Merged PRs per week" })
      .locator('rect[data-bar="Week of 2026-11-16"]');
    await expect(bar).toBeVisible();
    await expect(page.getByText("may be cut off: issues.")).toBeVisible();
    const colors = await page.evaluate(() => {
      const css = (sel: string, prop: string) => {
        const el = document.querySelector(sel);
        if (el === null) throw new Error(`missing ${sel}`);
        return getComputedStyle(el).getPropertyValue(prop);
      };
      return {
        bg: css("body", "background-color"),
        bar: css(
          'svg[aria-label="Merged PRs per week"] rect[data-bar="Week of 2026-11-16"]',
          "fill",
        ),
      };
    });
    expect(colors.bg).toBe(scheme === "dark" ? "rgb(18, 18, 18)" : "rgb(245, 245, 245)");
    // Every text style: headings, the secondary text, header and body cells, the Alert.
    await expectReadable(page, ["h1", "h2", "main p", "th", "td", '[role="alert"]']);
    expect(contrast(colors.bar, colors.bg)).toBeGreaterThanOrEqual(3);
  });

  test(`the invalid state's warning has enough contrast (${scheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await serve(page, snapshot({ version: 2 }));
    await page.goto("./dashboard/");
    await expect(page.getByText(INVALID)).toBeVisible();
    await expectReadable(page, ["h1", '[role="alert"]']);
  });

  test(`the invalid judge report's warning has enough contrast (${scheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await serveReport(page, report({ tier: "exam" }));
    await serve(page, snapshot());
    await page.goto("./dashboard/");
    await expect(
      page.getByText("Invalid: the judge report doesn't have the expected format."),
    ).toBeVisible();
    await expectReadable(page, ['[role="alert"]']);
  });
}
