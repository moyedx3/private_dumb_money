import { defineConfig, devices } from "@playwright/test";

const localViteUrl = "http://127.0.0.1:5173";

export default defineConfig({
  testDir: "./tests",
  webServer: {
    command: "npm run dev -- --host 127.0.0.1",
    url: localViteUrl,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000
  },
  use: {
    baseURL: localViteUrl,
    trace: "on-first-retry"
  },
  projects: [
    {
      name: "chromium",
      use: {
        ...devices["Desktop Chrome"]
      }
    }
  ]
});
