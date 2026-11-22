import { expect, test } from "../fixtures";
import { PROBE_ORIGIN, serve } from "./fixture-pages";

// A console warning in the page fails the test (errors are checked the same way).
test("console warning", { tag: "@e2e.web.harness_probe" }, async ({ page }) => {
  await serve(page);
  await page.goto(PROBE_ORIGIN);
  await page.evaluate(() => console.warn("probe warning"));
  await expect(page.getByText("clean")).toBeVisible();
});
