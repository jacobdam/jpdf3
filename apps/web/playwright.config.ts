import { defineConfig, devices } from "@playwright/test";

// E2E for the web app (specs/ui-web/app-skeleton.md, docs/adr/0002). Runs against the
// production build (`vite preview` of dist/), never the dev server, so the CSP is enforced.
const PORT = 4173;

export default defineConfig({
  testDir: "./e2e",
  forbidOnly: true,
  retries: 0,
  fullyParallel: true,
  reporter: [["list"], ["./e2e/journey-reporter.ts"]],
  use: {
    baseURL: `http://127.0.0.1:${PORT}/`,
    trace: "retain-on-failure",
    serviceWorkers: "block",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: `node node_modules/vite/bin/vite.js preview --host 127.0.0.1 --port ${PORT} --strictPort`,
    url: `http://127.0.0.1:${PORT}/`,
    reuseExistingServer: false,
    timeout: 60_000,
  },
});
