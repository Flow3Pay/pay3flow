import { defineConfig, devices } from "@playwright/test";

const port = process.env.PLAYWRIGHT_PORT ?? "3000";
const baseURL = `http://127.0.0.1:${port}`;
const sharePort = process.env.PLAYWRIGHT_SHARE_PORT ?? "8091";
const chromiumExecutable = process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE;

export default defineConfig({
  testDir: "./tests",
  fullyParallel: true,
  retries: process.env.CI ? 2 : 0,
  reporter: "list",
  use: {
    baseURL,
    trace: "on-first-retry",
    launchOptions: chromiumExecutable ? { executablePath: chromiumExecutable } : undefined,
  },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "mobile", use: { ...devices["Pixel 7"] } },
  ],
  webServer: [{
    command: "node tests/share-backend-fixture.mjs",
    url: `http://127.0.0.1:${sharePort}/health`,
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
  }, {
    env: { SHARE_API_URL: `http://127.0.0.1:${sharePort}`, PUBLIC_AURORA_INTENTS_API_KEY: "playwright-widget-key" },
    command: process.env.PLAYWRIGHT_USE_PREBUILT_VENDOR === "1"
      ? `test -f static/vendor/hex-hot-kit.js && node node_modules/vite/bin/vite.js --host 127.0.0.1 --port ${port}`
      : `pnpm run dev --host 127.0.0.1 --port ${port}`,
    url: baseURL,
    reuseExistingServer: !process.env.CI,
    timeout: Number(process.env.PLAYWRIGHT_STARTUP_TIMEOUT ?? 120_000),
  }],
});
