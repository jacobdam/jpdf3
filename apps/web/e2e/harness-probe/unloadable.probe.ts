import { test } from "@playwright/test";

// Registers a test with the probe ID, then fails to load. The run fails, and no ID may pass
// (Playwright 1.65 then runs no test at all, so passing.probe.ts reports nothing either).
test("dropped with its file", { tag: "@e2e.web.harness_probe" }, () => {});

throw new Error("deliberately unloadable probe file");
