import { writeFileSync } from "node:fs";
import type { FullResult, Reporter, Suite, TestCase, TestError } from "@playwright/test/reporter";

// Maps Playwright tests to journey test IDs (docs/adr/0002). A test carries its IDs as tags,
// `{ tag: "@e2e.web.<id>" }`. Writes one `<id>\t<pass|fail|skip>` line per ID to the file
// named by JPDF_E2E_RESULTS; `cargo xtask ci linux` validates it and writes e2e.json.

const TAG = /^@(e2e\.web\.[a-z0-9_]+)$/;
export type Status = "pass" | "fail" | "skip";
const RANK: Record<Status, number> = { pass: 0, skip: 1, fail: 2 };

type Test = Pick<TestCase, "outcome" | "expectedStatus" | "results">;

export function statusOf(test: Test): Status {
  // Playwright's outcome() counts an interrupted result as "skipped" (or ignores it next to
  // another result); ADR 0002 says interrupted is a failure.
  if (test.results.some((r) => r.status === "interrupted")) return "fail";
  const outcome = test.outcome();
  if (outcome === "skipped") return "skip";
  // An "expected failure" (test.fail) or a flaky pass never counts as a pass.
  if (outcome === "expected" && test.expectedStatus === "passed") return "pass";
  return "fail";
}

/** The worst status wins per ID: fail > skip > pass. */
export function collect(tests: readonly (Test & Pick<TestCase, "tags">)[]): Map<string, Status> {
  const out = new Map<string, Status>();
  for (const test of tests) {
    const status = statusOf(test);
    for (const tag of test.tags) {
      const id = TAG.exec(tag)?.[1];
      if (id === undefined) continue;
      const prev = out.get(id);
      if (prev === undefined || RANK[status] > RANK[prev]) out.set(id, status);
    }
  }
  return out;
}

/**
 * Tags that look meant as a journey ID but aren't a valid web one: any tag starting with
 * `@e2e` (any case) that TAG rejects, such as `@e2e.web.Bad`, `@e2e.web.a.b`, `@E2E.web.a` or
 * `@e2e.macos.a`. `collect` ignores them, so without this check their test would count for
 * nothing and no one would notice (#76 item 4). Returns `"<tag>" on "<test title>"` lines.
 */
export function malformedTags(tests: readonly Pick<TestCase, "tags" | "title">[]): string[] {
  const out: string[] = [];
  for (const test of tests) {
    for (const tag of test.tags) {
      if (/^@e2e/i.test(tag) && !TAG.test(tag))
        out.push(`${JSON.stringify(tag)} on ${JSON.stringify(test.title)}`);
    }
  }
  return out;
}

/**
 * A run that didn't pass as a whole (a spec file that failed to load, a throwing hook or
 * global setup, a reported error, a timed-out or interrupted run) reports no ID as passing:
 * it may have dropped tests of an ID that another test passed (#76).
 */
export function finalize(results: Map<string, Status>, runPassed: boolean): Map<string, Status> {
  if (runPassed) return results;
  return new Map([...results].map(([id, s]) => [id, s === "pass" ? "fail" : s]));
}

export default class JourneyReporter implements Reporter {
  private suite: Suite | undefined;
  private errored = false;

  onBegin(_config: unknown, suite: Suite): void {
    this.suite = suite;
  }

  onError(_error: TestError): void {
    this.errored = true;
  }

  onEnd(result: FullResult): void {
    const path = process.env.JPDF_E2E_RESULTS;
    if (path === undefined || path === "") return;
    const tests = this.suite?.allTests() ?? [];
    // A malformed journey tag makes the run fail like a reported error: no ID passes.
    const malformed = malformedTags(tests);
    for (const m of malformed)
      process.stderr.write(`journey-reporter: malformed journey tag ${m}\n`);
    // Tests that never ran (interrupted run) report "skipped", so they can't pass.
    const results = finalize(
      collect(tests),
      result.status === "passed" && !this.errored && malformed.length === 0,
    );
    const lines = [...results.entries()]
      .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
      .map(([id, status]) => `${id}\t${status}\n`);
    writeFileSync(path, lines.join(""));
  }

  printsToStdio(): boolean {
    return false;
  }
}
