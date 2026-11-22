import { writeFileSync } from "node:fs";
import type { FullResult, Reporter, Suite, TestCase } from "@playwright/test/reporter";

// Maps Playwright tests to journey test IDs (docs/adr/0002). A test carries its IDs as tags,
// `{ tag: "@e2e.web.<id>" }`. Writes one `<id>\t<pass|fail|skip>` line per ID to the file
// named by JPDF_E2E_RESULTS; `cargo xtask ci linux` validates it and writes e2e.json.

const TAG = /^@(e2e\.web\.[a-z0-9_]+)$/;
export type Status = "pass" | "fail" | "skip";
const RANK: Record<Status, number> = { pass: 0, skip: 1, fail: 2 };

export function statusOf(test: Pick<TestCase, "outcome" | "expectedStatus">): Status {
  const outcome = test.outcome();
  if (outcome === "skipped") return "skip";
  // An "expected failure" (test.fail) or a flaky pass never counts as a pass.
  if (outcome === "expected" && test.expectedStatus === "passed") return "pass";
  return "fail";
}

/** The worst status wins per ID: fail > skip > pass. */
export function collect(
  tests: readonly Pick<TestCase, "outcome" | "expectedStatus" | "tags">[],
): Map<string, Status> {
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

export default class JourneyReporter implements Reporter {
  private suite: Suite | undefined;

  onBegin(_config: unknown, suite: Suite): void {
    this.suite = suite;
  }

  onEnd(_result: FullResult): void {
    const path = process.env.JPDF_E2E_RESULTS;
    if (path === undefined || path === "") return;
    // Tests that never ran (interrupted run) report "skipped", so they can't pass.
    const results = collect(this.suite?.allTests() ?? []);
    const lines = [...results.entries()]
      .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
      .map(([id, status]) => `${id}\t${status}\n`);
    writeFileSync(path, lines.join(""));
  }

  printsToStdio(): boolean {
    return false;
  }
}
