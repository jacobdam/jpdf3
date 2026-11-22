import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import type { FullResult, Suite, TestError } from "@playwright/test/reporter";
import JourneyReporter, { collect, finalize, malformedTags, statusOf } from "./journey-reporter";

// Unit tests for the test-ID mapping (docs/adr/0002) and the run-failure rule (#76). No
// browser needed.
type T = Parameters<typeof collect>[0][number] & Parameters<typeof malformedTags>[0][number];
const t = (
  outcome: string,
  tags: string[],
  expectedStatus = "passed",
  statuses: string[] = [],
): T =>
  ({
    outcome: () => outcome,
    expectedStatus,
    tags,
    title: "t",
    results: statuses.map((status) => ({ status })),
  }) as unknown as T;

test("outcomes map to pass, fail and skip", () => {
  expect(statusOf(t("expected", [], "passed", ["passed"]))).toBe("pass");
  expect(statusOf(t("unexpected", [], "passed", ["failed"]))).toBe("fail");
  expect(statusOf(t("flaky", [], "passed", ["failed", "passed"]))).toBe("fail");
  expect(statusOf(t("skipped", [], "passed", ["skipped"]))).toBe("skip");
  // test.fail(): an expected failure is not a pass.
  expect(statusOf(t("expected", [], "failed", ["failed"]))).toBe("fail");
});

test("an interrupted test is a failure (ADR 0002), not a skip", () => {
  // Playwright's own outcome() says "skipped" for these.
  expect(statusOf(t("skipped", [], "passed", ["interrupted"]))).toBe("fail");
  expect(statusOf(t("expected", [], "passed", ["passed", "interrupted"]))).toBe("fail");
});

test("the worst status wins per ID and foreign tags are ignored", () => {
  const got = collect([
    t("expected", ["@e2e.web.a", "@e2e.web.b"]),
    t("unexpected", ["@e2e.web.a"]),
    t("skipped", ["@e2e.web.b"]),
    t("expected", ["@e2e.web.c", "@slow", "@e2e.macos.c", "@e2e.web.Bad", "@e2e.web.c.d"]),
  ]);
  expect([...got.entries()].sort()).toEqual([
    ["e2e.web.a", "fail"],
    ["e2e.web.b", "skip"],
    ["e2e.web.c", "pass"],
  ]);
});

test("a failed run reports no pass", () => {
  const results = new Map([
    ["e2e.web.a", "pass"],
    ["e2e.web.b", "skip"],
    ["e2e.web.c", "fail"],
  ] as const);
  expect([...finalize(new Map(results), true)]).toEqual([...results]);
  expect([...finalize(new Map(results), false)]).toEqual([
    ["e2e.web.a", "fail"],
    ["e2e.web.b", "skip"],
    ["e2e.web.c", "fail"],
  ]);
});

test("tags meant as journey IDs but malformed are found", () => {
  expect(
    malformedTags([
      t("expected", ["@e2e.web.a", "@slow", "@e2e.web.Bad", "@e2e.web.c.d"]),
      t("expected", ["@E2E.web.a", "@e2e.macos.a", "@e2e.web.", "@e2e-web-a", "@e2e"]),
      t("expected", ["@e2ex", "@xe2e.web.a", "@smoke_e2e"]),
    ]),
  ).toEqual([
    '"@e2e.web.Bad" on "t"',
    '"@e2e.web.c.d" on "t"',
    '"@E2E.web.a" on "t"',
    '"@e2e.macos.a" on "t"',
    '"@e2e.web." on "t"',
    '"@e2e-web-a" on "t"',
    '"@e2e" on "t"',
    '"@e2ex" on "t"',
  ]);
  // Well-formed tags and tags that don't start with @e2e are not reported.
  expect(malformedTags([t("expected", ["@e2e.web.a_1", "@slow", "@xe2e.web.a"])])).toEqual([]);
});

/** Drives the reporter class directly: one passing test with `tags`, then the run events. */
function report(
  status: FullResult["status"],
  errored: boolean,
  tags: string[] = ["@e2e.web.a"],
): string {
  const dir = mkdtempSync(join(tmpdir(), "jpdf-reporter-"));
  const file = join(dir, "r.tsv");
  const saved = process.env.JPDF_E2E_RESULTS;
  process.env.JPDF_E2E_RESULTS = file;
  try {
    const reporter = new JourneyReporter();
    const suite = { allTests: () => [t("expected", tags, "passed", ["passed"])] };
    reporter.onBegin(undefined, suite as unknown as Suite);
    if (errored) reporter.onError({ message: "boom" } as TestError);
    reporter.onEnd({ status } as FullResult);
    return readFileSync(file, "utf8");
  } finally {
    if (saved === undefined) delete process.env.JPDF_E2E_RESULTS;
    else process.env.JPDF_E2E_RESULTS = saved;
    rmSync(dir, { recursive: true, force: true });
  }
}

test("the reporter downgrades on a failed run status and on any reported error", () => {
  expect(report("passed", false)).toBe("e2e.web.a\tpass\n");
  for (const status of ["failed", "timedout", "interrupted"] as const) {
    expect(report(status, false), status).toBe("e2e.web.a\tfail\n");
  }
  // onError alone, even if Playwright still called the run passed.
  expect(report("passed", true)).toBe("e2e.web.a\tfail\n");
});

test("a malformed journey tag on any test turns every pass into a fail (#76 item 4)", () => {
  expect(report("passed", false, ["@e2e.web.a", "@slow"])).toBe("e2e.web.a\tpass\n");
  expect(report("passed", false, ["@e2e.web.a", "@e2e.web.A"])).toBe("e2e.web.a\tfail\n");
  // Only malformed tags: no ID at all (xtask then fails on "no journey test ids").
  expect(report("passed", false, ["@e2e.web.a-b"])).toBe("");
});

/**
 * Runs the probe project (e2e/harness-probe) in a child Playwright with the journey reporter.
 * `files` filters the probe files by name; returns the exit code and the reporter's file.
 */
function probe(files: string[]): { code: number | null; tsv: string; log: string } {
  const dir = mkdtempSync(join(tmpdir(), "jpdf-probe-"));
  const file = join(dir, "r.tsv");
  // Paths from this file's location, so the probe works wherever Playwright was started.
  const web = join(import.meta.dirname, "..");
  try {
    const run = spawnSync(
      process.execPath,
      [
        join(web, "node_modules", "@playwright", "test", "cli.js"),
        "test",
        "-c",
        join(web, "e2e", "harness-probe", "playwright.config.ts"),
        ...files,
      ],
      {
        cwd: web,
        encoding: "utf8",
        timeout: 60_000,
        env: { ...process.env, JPDF_E2E_RESULTS: file, JPDF_PROBE_OUT: join(dir, "out"), CI: "1" },
      },
    );
    let tsv = "";
    try {
      tsv = readFileSync(file, "utf8");
    } catch {
      tsv = "<no results file>";
    }
    return { code: run.status, tsv, log: `${run.stdout}\n${run.stderr}` };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test.describe("a real Playwright run with the journey reporter", () => {
  test.describe.configure({ timeout: 120_000 });

  test("a clean probe run passes (control)", () => {
    const r = probe(["passing.probe"]);
    expect(r.code, r.log).toBe(0);
    expect(r.tsv, r.log).toBe("e2e.web.harness_probe\tpass\n");
  });

  test("an unloadable spec file leaves no passing ID", () => {
    // Playwright 1.65 runs no test at all when a file fails to load, so the reporter writes
    // no ID (and xtask fails on "no journey test ids"). This is a tripwire for that
    // Playwright behavior, not a check of the reporter: any reporter passes it today. The
    // downgrade itself is checked by the hook probe below and the onError unit test; the
    // exact empty file makes a Playwright that starts running the other files fail here.
    const r = probe(["passing.probe", "unloadable.probe"]);
    expect(r.code, r.log).not.toBe(0);
    expect(r.tsv, r.log).toBe("");
  });

  test("a throwing hook in another file turns the passing ID into a fail", () => {
    const r = probe(["passing.probe", "hook.probe"]);
    expect(r.code, r.log).not.toBe(0);
    expect(r.tsv, r.log).toBe("e2e.web.harness_probe\tfail\n");
  });

  // The shared fixture (./fixtures.ts) fails a browser test that the old per-test checks let
  // through when a test forgot them (#76 item 5).
  test("a clean page passes the shared fixture (control)", () => {
    const r = probe(["fixture-clean.probe"]);
    expect(r.code, r.log).toBe(0);
    expect(r.tsv, r.log).toBe("e2e.web.harness_probe\tpass\n");
  });

  for (const [file, what] of [
    ["fixture-csp-stay.probe", "a CSP violation on the last page"],
    ["fixture-csp-away.probe", "a CSP violation on a page left earlier"],
    ["fixture-foreign.probe", "a foreign request"],
    ["fixture-console.probe", "a console warning"],
    ["fixture-uncaught.probe", "an uncaught error in the page"],
  ] as const) {
    test(`the shared fixture fails a test on ${what}`, () => {
      const r = probe([file]);
      expect(r.code, r.log).not.toBe(0);
      expect(r.tsv, r.log).toBe("e2e.web.harness_probe\tfail\n");
    });
  }
});
