import { test } from "@playwright/test";

// An untagged test whose hook throws: the tagged test in passing.probe.ts still passes, but the
// run fails, so no ID may pass.
test.beforeAll(() => {
  throw new Error("deliberately failing hook");
});

test("never runs", () => {});
