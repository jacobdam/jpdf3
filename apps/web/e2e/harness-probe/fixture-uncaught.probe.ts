import { expect, test } from "../fixtures";
import { PROBE_ORIGIN, serve } from "./fixture-pages";

// An uncaught error in the page fails the test, even with every console message allowed.
test("uncaught error", { tag: "@e2e.web.harness_probe" }, async ({ page, watch }) => {
  watch.allow((p) => p.startsWith("console "));
  await serve(page);
  await page.goto(PROBE_ORIGIN);
  await page.evaluate(() => {
    setTimeout(() => {
      throw new Error("probe uncaught");
    }, 0);
  });
  await expect(page.getByText("clean")).toBeVisible();
  // Let the timer fire and the pageerror event arrive before the fixture checks.
  await page.waitForTimeout(200);
});
