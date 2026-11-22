import { expect, test } from "@playwright/test";
import { collect, statusOf } from "./journey-reporter";

// Unit tests for the test-ID mapping (docs/adr/0002). No browser needed.
type T = Parameters<typeof collect>[0][number];
const t = (outcome: string, tags: string[], expectedStatus = "passed"): T =>
  ({ outcome: () => outcome, expectedStatus, tags }) as unknown as T;

test("outcomes map to pass, fail and skip", () => {
  expect(statusOf(t("expected", []))).toBe("pass");
  expect(statusOf(t("unexpected", []))).toBe("fail");
  expect(statusOf(t("flaky", []))).toBe("fail");
  expect(statusOf(t("skipped", []))).toBe("skip");
  // test.fail(): an expected failure is not a pass.
  expect(statusOf(t("expected", [], "failed"))).toBe("fail");
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
