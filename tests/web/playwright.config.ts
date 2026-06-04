import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  use: {
    baseURL: "http://127.0.0.1:4173",
  },
  webServer: [
    {
      command: "cargo run -p knowledge-server",
      port: 4001,
      reuseExistingServer: true,
      cwd: "../..",
      env: {
        KNOWLEDGE_BIND_ADDR: "127.0.0.1:4001",
        KNOWLEDGE_DATABASE_URL: "sqlite://playwright.db",
      },
    },
    {
      command: "npm run dev --workspace @knowledge/admin -- --host 127.0.0.1 --port 4173",
      port: 4173,
      reuseExistingServer: true,
      cwd: "../..",
    },
  ],
});
