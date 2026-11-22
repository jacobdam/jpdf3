import { expect, test } from "../fixtures";
import { PROBE_ORIGIN, serve } from "./fixture-pages";

// A request outside the configured origin fails the test (here a page on another host).
test("foreign request", { tag: "@e2e.web.harness_probe" }, async ({ page }) => {
  await serve(page);
  await page.route("http://elsewhere.test/**", (route) =>
    route.fulfill({ contentType: "text/html", body: "<title>x</title>" }),
  );
  await page.goto(PROBE_ORIGIN);
  await page.goto("http://elsewhere.test/");
  await expect(page).toHaveTitle("x");
});
