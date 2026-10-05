import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  testMatch: "web.spec.js",
  outputDir: "target/web-tests",
  workers: 1,
  timeout: 60_000,
  use: {
    baseURL: "http://127.0.0.1:4173",
    viewport: { width: 1280, height: 800 },
    deviceScaleFactor: 1,
    channel: process.env.PLAYWRIGHT_CHANNEL ?? "msedge",
    screenshot: "only-on-failure",
  },
  webServer: {
    command: "node scripts/serve-web.mjs --dir outputs --port 4173",
    url: "http://127.0.0.1:4173/web/",
    reuseExistingServer: false,
  },
});
