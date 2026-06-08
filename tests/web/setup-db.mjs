import { execFileSync } from "node:child_process";
import { randomUUID } from "node:crypto";

const database = `knowledge_web_${randomUUID().replaceAll("-", "")}`;
const container = process.env.KNOWLEDGE_TEST_POSTGRES_CONTAINER ?? "knowledge-postgres";
const appTemplate =
  process.env.KNOWLEDGE_TEST_POSTGRES_APP_URL_TEMPLATE ??
  "postgres://postgres:postgres@127.0.0.1:55432/{database}?sslmode=disable";

execFileSync(
  "docker",
  [
    "exec",
    container,
    "psql",
    "-U",
    "postgres",
    "-d",
    "postgres",
    "-c",
    `CREATE DATABASE ${database};`,
  ],
  { stdio: "ignore" },
);

process.stdout.write(appTemplate.replace("{database}", database));
