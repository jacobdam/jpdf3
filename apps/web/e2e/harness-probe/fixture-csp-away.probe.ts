import { expect, test } from "../fixtures";
import { PROBE_ORIGIN, serve } from "./fixture-pages";

// A CSP violation on a page the test navigated away from still fails the test.
test("violation on an earlier page", { tag: "@e2e.web.harness_probe" }, async ({ page, watch }) => {
  watch.allow((p) => p.startsWith("console "));
  await serve(page);
  await page.goto(`${PROBE_ORIGIN}violates`);
  await expect(page).toHaveTitle("v");
  await page.goto(PROBE_ORIGIN);
  await expect(page.getByText("clean")).toBeVisible();
});
