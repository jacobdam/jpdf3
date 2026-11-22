import { expect, test } from "../fixtures";
import { PROBE_ORIGIN, serve } from "./fixture-pages";

// Control for the fixture probes: a clean page passes the shared checks.
test("clean page", { tag: "@e2e.web.harness_probe" }, async ({ page }) => {
  await serve(page);
  await page.goto(PROBE_ORIGIN);
  await expect(page.getByText("clean")).toBeVisible();
});
