import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test } from "@playwright/test";

const fixturesDir = join(dirname(fileURLToPath(import.meta.url)), "..", "fixtures", "uploads");
const singleUploadPath = join(fixturesDir, "attention.md");
const folderUploadPath = join(fixturesDir, "team-a");

test("admin can inspect project operations data", async ({ page }) => {
  await page.goto("/login");
  await page.getByLabel("Username").fill("admin");
  await page.getByLabel("Password").fill("secret-password");
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page.getByRole("heading", { name: "Projects" })).toBeVisible();
  await page.getByRole("button", { name: "Create Demo Project" }).click();
  await expect(page.getByRole("heading", { name: /seed-project-/ })).toBeVisible();

  await page.getByRole("link", { name: "Sources", exact: true }).click();
  const attentionSource = page.getByRole("listitem").filter({ hasText: "raw/sources/attention.md" });
  const nestedChildSource = page.getByRole("listitem").filter({ hasText: "raw/sources/team-a/child.md" });
  await page.getByLabel("Files to Upload").setInputFiles(singleUploadPath);
  await page.getByRole("button", { name: "Upload Files" }).click();
  await page.getByRole("link", { name: "Tasks", exact: true }).click();
  const importTask = page.getByRole("listitem").filter({ hasText: "Imported attention.md" });
  await expect(importTask).toBeVisible();
  await page.getByRole("link", { name: "Sources", exact: true }).click();
  await expect(attentionSource).toBeVisible();

  await page.getByLabel("Folder to Import").setInputFiles(folderUploadPath);
  await page.getByRole("button", { name: "Import Folder" }).click();
  await expect(nestedChildSource).toBeVisible();
  await expect(page.getByText("raw/sources/team-a/docs/peer.md").first()).toBeVisible();

  await attentionSource.getByRole("button", { name: "Ingest" }).click();
  await page.getByRole("link", { name: "Tasks", exact: true }).click();
  const ingestTask = page
    .getByRole("listitem")
    .filter({ hasText: "Ingest raw/sources/attention.md" });
  await expect(ingestTask).toBeVisible();
  await expect(ingestTask).toContainText("succeeded");

  await page.getByRole("link", { name: "Search", exact: true }).click();
  await page.getByLabel("Search Query").fill("relevant tokens");
  await page.getByRole("button", { name: "Run Search" }).click();
  await expect(page.getByText("wiki/sources/attention.md")).toBeVisible();

  await page.getByRole("link", { name: "Sources", exact: true }).click();
  await nestedChildSource.getByRole("button", { name: "Ingest" }).click();
  await page.getByRole("link", { name: "Tasks", exact: true }).click();
  const nestedIngestTask = page
    .getByRole("listitem")
    .filter({ hasText: "Ingest raw/sources/team-a/child.md" });
  await expect(nestedIngestTask).toContainText("succeeded");

  await page.getByRole("link", { name: "Graph", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Graph" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Attention" })).toBeVisible();

  await page.getByRole("link", { name: "Tasks", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Tasks" })).toBeVisible();
  await expect(page.getByText("raw/sources/attention.md").first()).toBeVisible();
  await expect(ingestTask).toBeVisible();
  await expect(page.getByText("raw/sources/team-a/child.md").first()).toBeVisible();

  await page.getByRole("link", { name: "Sources", exact: true }).click();
  await attentionSource.getByRole("button", { name: "Delete" }).click();
  await expect(page.getByText("raw/sources/attention.md").first()).not.toBeVisible();

  await page.getByRole("link", { name: "Tasks", exact: true }).click();
  const deleteTask = page.getByRole("listitem").filter({ hasText: "Deleted attention.md" });
  await expect(deleteTask).toBeVisible();
  await page.getByRole("link", { name: "Sources", exact: true }).click();
  await expect(page.getByText("raw/sources/attention.md").first()).not.toBeVisible();

  await page.getByRole("link", { name: "Settings", exact: true }).click();
  await page.getByLabel("Default Query Limit").fill("5");
  await page.getByRole("button", { name: "Save Settings" }).click();
  await expect(page.getByLabel("Default Query Limit")).toHaveValue("5");
});
