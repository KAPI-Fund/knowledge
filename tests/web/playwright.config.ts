import { execFileSync } from "node:child_process";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";

import { defineConfig } from "@playwright/test";

const configDir = dirname(fileURLToPath(import.meta.url));
const databaseUrl =
  process.env.KNOWLEDGE_DATABASE_URL ??
  execFileSync("node", ["./setup-db.mjs"], {
    cwd: configDir,
    encoding: "utf8",
  }).trim();

export default defineConfig({
  testDir: "./tests",
  workers: 1,
  use: {
    baseURL: "http://127.0.0.1:4173",
  },
  webServer: [
    {
      command: "node ./mock-openai.mjs",
      port: 18080,
      reuseExistingServer: false,
      cwd: ".",
      env: {
        KNOWLEDGE_MOCK_OPENAI_PORT: "18080",
      },
    },
    {
      command: "cargo run -p knowledge-server",
      port: 4001,
      reuseExistingServer: false,
      cwd: "../..",
      env: {
        KNOWLEDGE_BIND_ADDR: "127.0.0.1:4001",
        KNOWLEDGE_DATABASE_URL: databaseUrl,
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
