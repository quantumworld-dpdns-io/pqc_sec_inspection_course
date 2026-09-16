import { defineConfig, devices } from "@playwright/test";

/**
 * The e2e suite runs against a *running stack* (compose), not a dev server: the point is to
 * exercise the real API, worker and adapters, so `make dev` (or the CI compose step) must be
 * up first.
 */
export default defineConfig({
  testDir: "./e2e",
  timeout: 90_000,
  expect: { timeout: 15_000 },
  fullyParallel: false,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["html", { open: "never" }], ["list"]] : "list",
  use: {
    baseURL: process.env.E2E_BASE_URL ?? "http://localhost:8088",
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
});
