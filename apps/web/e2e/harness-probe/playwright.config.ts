import { defineConfig } from "@playwright/test";
import { PROBE_ORIGIN } from "./fixture-pages";

// A Playwright project that only journey-reporter.spec.ts runs, as a child process, to check
// that the journey reporter turns a failed run into failures (#76). Its files end in
// `.probe.ts`, so the app's own config (testDir ./e2e, default *.spec.ts match) never runs them.
export default defineConfig<{ origin: string }>({
  testDir: ".",
  testMatch: /\.probe\.ts$/,
  forbidOnly: true,
  retries: 0,
  workers: 1,
  reporter: [["../journey-reporter.ts"]],
  // The parent test gives each run its own directory, so it never writes into the app's
  // test-results/ (the parent run's artifacts) or another probe run's.
  outputDir: process.env.JPDF_PROBE_OUT ?? "probe-results",
  // The fixture probes (fixture-*.probe.ts) serve their pages from this origin.
  use: { origin: PROBE_ORIGIN },
});
