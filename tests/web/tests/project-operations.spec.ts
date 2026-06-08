import { expect, test } from "@playwright/test";

test("admin can inspect project operations data", async ({ page }) => {
  await page.goto("/login");
  await page.getByLabel("Username").fill("admin");
  await page.getByLabel("Password").fill("secret-password");
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page.getByRole("heading", { name: "Projects" })).toBeVisible();
  await page.getByRole("button", { name: "Create Demo Project" }).click();

  await page.getByRole("link", { name: "seed-project" }).last().click();
  await expect(page.getByRole("heading", { name: "seed-project" })).toBeVisible();

  await page.getByRole("link", { name: "Sources" }).click();
  await page.getByLabel("File Name").fill("attention.md");
  await page
    .getByLabel("Markdown Content")
    .fill("# Attention\n\nAttention lets models focus on relevant tokens.");
  await page.getByRole("button", { name: "Import Source" }).click();
  await page.getByRole("link", { name: "Tasks" }).click();
  const importTask = page.getByRole("listitem").filter({ hasText: "Imported attention.md" });
  await expect(importTask).toBeVisible();
  await page.getByRole("link", { name: "Sources" }).click();
  await expect(page.getByText("raw/sources/attention.md").first()).toBeVisible();
  await page.getByRole("button", { name: "Ingest" }).click();
  await page.getByRole("link", { name: "Tasks" }).click();
  const ingestTask = page
    .getByRole("listitem")
    .filter({ hasText: "Ingest raw/sources/attention.md" });
  await expect(ingestTask).toBeVisible();
  await expect(ingestTask).toContainText("succeeded");

  await page.getByRole("link", { name: "Search" }).click();
  await page.getByLabel("Search Query").fill("relevant tokens");
  await page.getByRole("button", { name: "Run Search" }).click();
  await expect(page.getByText("wiki/sources/attention.md")).toBeVisible();

  await page.getByRole("link", { name: "Graph" }).click();
  await expect(page.getByRole("heading", { name: "Graph" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Attention" })).toBeVisible();

  await page.getByRole("link", { name: "Tasks" }).click();
  await expect(page.getByRole("heading", { name: "Tasks" })).toBeVisible();
  await expect(page.getByText("raw/sources/attention.md").first()).toBeVisible();
  await expect(ingestTask).toBeVisible();

  await page.getByRole("link", { name: "Sources" }).click();
  await page.getByRole("button", { name: "Delete" }).click();
  await expect(page.getByText("raw/sources/attention.md").first()).not.toBeVisible();

  await page.getByRole("link", { name: "Tasks" }).click();
  const deleteTask = page.getByRole("listitem").filter({ hasText: "Deleted attention.md" });
  await expect(deleteTask).toBeVisible();
  await page.getByRole("link", { name: "Sources" }).click();
  await expect(page.getByText("raw/sources/attention.md").first()).not.toBeVisible();

  await page.getByRole("link", { name: "Settings" }).click();
  await page.getByLabel("Default Query Limit").fill("5");
  await page.getByRole("button", { name: "Save Settings" }).click();
  await expect(page.getByLabel("Default Query Limit")).toHaveValue("5");
});
