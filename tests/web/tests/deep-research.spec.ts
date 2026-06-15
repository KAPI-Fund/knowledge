import { expect, test } from "@playwright/test";

test("admin queues a deep research task that completes successfully", async ({ page }) => {
  await signInAsAdmin(page);

  await page.getByRole("link", { name: "Settings" }).click();
  await page.getByLabel("Provider Mode", { exact: true }).fill("openai-compatible");
  await page.getByLabel("Language", { exact: true }).fill("en");
  await page.getByLabel("Provider Base URL", { exact: true }).fill("http://127.0.0.1:18080/v1");
  await page.getByLabel("Provider API Key", { exact: true }).fill("test-key");
  await page.getByLabel("Provider Model", { exact: true }).fill("mock-model");
  await page.getByLabel("Provider Timeout Seconds", { exact: true }).fill("30");
  await page.getByLabel(/Search Provider/i).selectOption("searxng");
  await page.getByLabel(/SearXNG Instance URL/i).fill("http://127.0.0.1:18080");
  await page.getByRole("button", { name: "Save Settings" }).click();

  await page.getByRole("link", { name: "Projects" }).click();
  await createProject(page, "deep-research-project");

  await page.getByRole("tab", { name: "Deep Research" }).click();
  await page.getByLabel("Topic").fill("Knowledge Graphs");
  await page.getByLabel("Search Queries").fill("knowledge graphs");
  await page.getByRole("button", { name: "Start Research" }).click();

  await expect(page.getByText("Deep research: Knowledge Graphs")).toBeVisible({ timeout: 10_000 });
  await expect(page.getByText("succeeded").first()).toBeVisible({ timeout: 30_000 });
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
