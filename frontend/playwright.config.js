import { defineConfig } from "@playwright/test";

const browser = process.platform === "win32"
  ? { browserName: "chromium", channel: "msedge" }
  : process.platform === "darwin"
    ? { browserName: "webkit" }
    : { browserName: "chromium" };

export default defineConfig({
  testDir: "./e2e",
  // Match Tauri's platform web engine: WebView2/Edge on Windows, WebKit on macOS.
  use: { baseURL: "http://localhost:5173", testIdAttribute: "data-test", ...browser },
  webServer: {
    command: "npm run dev",
    url: "http://localhost:5173",
    reuseExistingServer: true,
    timeout: 60000,
  },
});
