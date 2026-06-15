import { expect, test } from "@playwright/test";

test("admin configures searxng and runs a test search", async ({ page }) => {
  await signInAsAdmin(page);

  await page.getByRole("link", { name: "Settings" }).click();

  await page.getByLabel(/Search Provider/i).selectOption("searxng");
  await page.getByLabel(/SearXNG Instance URL/i).fill("http://127.0.0.1:18080");
  await page.getByRole("button", { name: /Save Settings/i }).click();

  await page.getByLabel(/Test Query/i).fill("knowledge graphs");
  await page.getByRole("button", { name: /Test Search/i }).click();

  await expect(page.getByText("Knowledge graphs explained")).toBeVisible({ timeout: 10_000 });
});

async function signInAsAdmin(page: import("@playwright/test").Page) {
  await page.goto("/login");
  await page.getByLabel("Username").fill("admin");
  await page.getByLabel("Password").fill("secret-password");
  await page.getByRole("button", { name: "Sign in" }).click();
  await page.waitForURL("**/projects");
}
