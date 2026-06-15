import { expect, test } from "@playwright/test";

test("admin can run a provider-backed query and save it to the wiki", async ({ page }) => {
  await signInAsAdmin(page);

  await page.getByRole("link", { name: "Settings" }).click();
  await page.getByLabel("Provider Mode", { exact: true }).fill("openai-compatible");
  await page.getByLabel("Language", { exact: true }).fill("en");
  await page.getByLabel("Provider Base URL", { exact: true }).fill("http://127.0.0.1:18080/v1");
  await page.getByLabel("Provider API Key", { exact: true }).fill("test-key");
  await page.getByLabel("Provider Model", { exact: true }).fill("mock-model");
  await page.getByLabel("Provider Timeout Seconds", { exact: true }).fill("30");
  await page.getByRole("button", { name: "Save Settings" }).click();

  await page.getByRole("link", { name: "Projects" }).click();
  await createProject(page, "query-project");
  await expect(page.getByRole("heading", { name: "query-project" })).toBeVisible();

  await page.getByRole("tab", { name: "Sources" }).click();
  await page.getByLabel("File Name").fill("attention.md");
  await page
    .getByLabel("Markdown Content")
    .fill("# Attention\n\nAttention lets models focus on relevant tokens.");
  await page.getByRole("button", { name: "Import Source" }).click();
  await expect(page.getByText("raw/sources/attention.md").first()).toBeVisible();
  await page.getByRole("button", { name: "Ingest" }).click();

  await page.getByRole("tab", { name: "Tasks" }).click();
  const ingestTask = page
    .getByRole("row")
    .filter({ hasText: "Ingest raw/sources/attention.md" });
  await expect(ingestTask).toContainText("succeeded");

  await page.getByRole("tab", { name: "Query", exact: true }).click();
  await page.getByLabel("Query").fill("What is attention?");
  await page.getByRole("button", { name: "Run Query" }).click();
  await expect(page.getByText("Attention focuses computation on relevant tokens.")).toBeVisible();
  await page.getByRole("button", { name: "Save To Wiki" }).click();
  await expect(page.getByText("Saved to wiki")).toBeVisible();
  await expect(page.getByText("wiki/queries/what-is-attention.md")).toBeVisible();

  await page.getByRole("tab", { name: "Search", exact: true }).click();
  await page.getByLabel("Search Query").fill("Attention focuses computation");
  await page.getByRole("button", { name: "Run Search" }).click();
  await expect(page.getByText("wiki/queries/what-is-attention.md")).toBeVisible();
});

async function signInAsAdmin(page: import("@playwright/test").Page) {
  await page.goto("/login");
  await page.getByLabel("Username").fill("admin");
  await page.getByLabel("Password").fill("secret-password");
  await page.getByRole("button", { name: "Sign in" }).click();
  await page.waitForURL("**/projects");
}

async function createProject(page: import("@playwright/test").Page, name: string) {
  await page.getByRole("button", { name: "Create Project" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.getByLabel("Name").fill(name);
  await page.getByRole("dialog").getByRole("button", { name: "Create Project" }).click();
  await page.waitForURL(/\/projects\/[^/]+$/);
}
