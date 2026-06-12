import { expect, test } from "@playwright/test";

test("admin can create a wiki page from the files page", async ({ page }) => {
  await signInAsAdmin(page);
  await createProject(page, "wiki-create-project");
  await expect(page.getByRole("heading", { name: "wiki-create-project" })).toBeVisible();

  await page.getByRole("tab", { name: "Files" }).click();
  await page.getByLabel("New Page Path").fill("wiki/concepts/new-topic.md");
  await page.getByRole("button", { name: "Create Page" }).click();

  await expect(page.getByText("wiki/concepts/new-topic.md").first()).toBeVisible();
  await expect(page.getByText("# new topic")).toBeVisible();
  await expect(page.getByRole("button", { name: "Edit" })).toBeVisible();
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
