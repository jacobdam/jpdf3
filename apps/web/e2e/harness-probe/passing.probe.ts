import { expect, test } from "@playwright/test";

test("the probe ID passes here", { tag: "@e2e.web.harness_probe" }, () => {
  expect(1).toBe(1);
});
