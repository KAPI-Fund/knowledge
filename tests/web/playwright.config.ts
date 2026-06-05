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
      reuseExistingServer: false,
      cwd: "../..",
      env: {
        KNOWLEDGE_BIND_ADDR: "127.0.0.1:4001",
        KNOWLEDGE_DATABASE_URL:
          process.env.KNOWLEDGE_DATABASE_URL ??
          "postgres://postgres:postgres@127.0.0.1:55432/knowledge?sslmode=disable",
        KNOWLEDGE_REDIS_URL: process.env.KNOWLEDGE_REDIS_URL ?? "redis://127.0.0.1:56379/",
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
