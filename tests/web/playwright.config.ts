import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  use: {
    baseURL: "http://127.0.0.1:4173",
  },
  webServer: {
    command: "npm run dev --workspace @knowledge/admin -- --host 127.0.0.1 --port 4173",
    port: 4173,
    reuseExistingServer: true,
    cwd: "../..",
  },
});
