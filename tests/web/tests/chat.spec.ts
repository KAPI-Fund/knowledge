import { expect, test } from "@playwright/test";

test("chat streams an answer into the conversation", async ({ page }) => {
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
  await createProject(page, "chat-project");
  await expect(page.getByRole("heading", { name: "chat-project" })).toBeVisible();

  await page.getByRole("tab", { name: "Chat" }).click();

  await page.getByRole("button", { name: "New conversation" }).click();
  await expect(
    page.locator("aside ul button").filter({ hasText: "New conversation" }).first(),
  ).toBeVisible();
  await page.getByLabel("Chat message").fill("What is attention?");
  await page.getByRole("button", { name: "Send" }).click();

  await expect(page.locator('[data-role="assistant"]').last()).toContainText(
    "Attention focuses computation on relevant tokens.",
    { timeout: 15_000 },
  );

  await page.reload();
  await page.getByRole("tab", { name: "Chat" }).click();
  await page.locator("aside ul button").filter({ hasText: "New conversation" }).first().click();
  await expect(page.locator('[data-role="assistant"]').last()).toContainText(
    "Attention focuses computation on relevant tokens.",
  );
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
