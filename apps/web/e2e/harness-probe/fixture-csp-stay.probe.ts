import { expect, test } from "../fixtures";
import { PROBE_ORIGIN, serve } from "./fixture-pages";

// A CSP violation on the page the test ends on fails the test, even with every console
// message allowed: the violation itself is checked, not only its console error.
test("violation on the last page", { tag: "@e2e.web.harness_probe" }, async ({ page, watch }) => {
  watch.allow((p) => p.startsWith("console "));
  await serve(page);
  await page.goto(`${PROBE_ORIGIN}violates`);
  await expect(page).toHaveTitle("v");
});
