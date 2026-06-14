import { expect, test } from "@playwright/test";

test("admin mints, uses, and revokes an api token", async ({ page, playwright }) => {
  await signInAsAdmin(page);

  await page.getByRole("link", { name: "API Tokens" }).click();
  await page.getByLabel("Token Name").fill("e2e-token");
  await page.getByRole("button", { name: "Mint Token" }).click();

  const tokenCode = page.locator("code").first();
  await expect(tokenCode).toBeVisible();
  const token = (await tokenCode.textContent())?.trim() ?? "";
  expect(token.length).toBeGreaterThanOrEqual(43);

  // Use a cookie-free request context so the Bearer token is the only credential.
  const bearer = await playwright.request.newContext({
    baseURL: "http://127.0.0.1:4173",
    extraHTTPHeaders: { authorization: `Bearer ${token}` },
  });

  const projects = await bearer.get("/api/projects");
  expect(projects.ok()).toBeTruthy();

  await page.getByRole("button", { name: "Dismiss" }).click();

  await page.getByRole("button", { name: "Revoke" }).click();

  await expect(page.getByText("revoked").first()).toBeVisible({ timeout: 5_000 });

  const denied = await bearer.get("/api/projects");
  expect(denied.status()).toBe(401);

  await bearer.dispose();
});

async function signInAsAdmin(page: import("@playwright/test").Page) {
  await page.goto("/login");
  await page.getByLabel("Username").fill("admin");
  await page.getByLabel("Password").fill("secret-password");
  await page.getByRole("button", { name: "Sign in" }).click();
  await page.waitForURL("**/projects");
}
