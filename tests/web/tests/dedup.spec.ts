import { expect, test } from "@playwright/test";

test("admin detects and merges duplicate wiki pages", async ({ page }) => {
  await signInAsAdmin(page);

  await page.getByRole("link", { name: "Settings" }).click();
  await page.getByLabel("Provider Mode", { exact: true }).fill("openai-compatible");
  await page.getByLabel("Language", { exact: true }).fill("en");
  await page.getByLabel("Provider Base URL", { exact: true }).fill("http://127.0.0.1:18080/v1");
  await page.getByLabel("Provider API Key", { exact: true }).fill("test-key");
  await page.getByLabel("Provider Model", { exact: true }).fill("mock-model");
  await page.getByLabel("Provider Embedding Model", { exact: true }).fill("mock-embedding");
  await page.getByLabel("Provider Timeout Seconds", { exact: true }).fill("30");
  await page.getByRole("button", { name: "Save Settings" }).click();

  await page.getByRole("link", { name: "Projects" }).click();
  await createProject(page, "dedup-project");
  const projectId = page.url().split("/").pop() ?? "";

  await seedWikiPage(
    page,
    projectId,
    "wiki/concepts/attention.md",
    [
      "---",
      "title: Attention",
      "type: concept",
      "description: How attention weighs token relevance.",
      "---",
      "",
      "Attention weighs token relevance.",
    ].join("\n"),
  );
  await seedWikiPage(
    page,
    projectId,
    "wiki/concepts/attention-mechanism.md",
    [
      "---",
      "title: Attention Mechanism",
      "type: concept",
      "description: The attention mechanism in transformers.",
      "---",
      "",
      "The attention mechanism scores pairwise token relevance.",
    ].join("\n"),
  );
  await seedWikiPage(
    page,
    projectId,
    "wiki/concepts/rope.md",
    [
      "---",
      "title: RoPE",
      "type: concept",
      "description: Rotary position embedding.",
      "---",
      "",
      "RoPE rotates query and key vectors by position.",
    ].join("\n"),
  );

  await page.getByRole("tab", { name: "Dedup" }).click();
  await page.getByRole("button", { name: "Detect Duplicates" }).click();

  await expect(page.getByText("attention / attention-mechanism")).toBeVisible({
    timeout: 30_000,
  });
  await expect(page.getByText("Both describe the attention mechanism.")).toBeVisible();

  await page.getByLabel("Canonical Slug").selectOption("attention-mechanism");
  await page.getByRole("button", { name: "Merge" }).click();

  await expect(page.getByText("No duplicate candidates")).toBeVisible({ timeout: 30_000 });

  const csrf = await page.evaluate(() => window.sessionStorage.getItem("knowledge.csrfToken"));
  const canonical = await page.request.get(
    `/api/projects/${projectId}/files/content?path=${encodeURIComponent(
      "wiki/concepts/attention-mechanism.md",
    )}`,
    { headers: { "x-csrf-token": csrf ?? "" } },
  );
  expect(canonical.ok()).toBeTruthy();
  const canonicalBody = await canonical.json();
  expect(canonicalBody.content).toContain(
    "Attention focuses computation on relevant tokens across the sequence.",
  );

  const deleted = await page.request.get(
    `/api/projects/${projectId}/files/content?path=${encodeURIComponent(
      "wiki/concepts/attention.md",
    )}`,
    { headers: { "x-csrf-token": csrf ?? "" } },
  );
  expect(deleted.ok()).toBeFalsy();
});

async function seedWikiPage(
  page: import("@playwright/test").Page,
  projectId: string,
  path: string,
  content: string,
) {
  const csrf = await page.evaluate(() => window.sessionStorage.getItem("knowledge.csrfToken"));
  const response = await page.request.put(`/api/projects/${projectId}/files/content`, {
    data: { path, content },
    headers: { "x-csrf-token": csrf ?? "" },
  });
  if (!response.ok()) {
    throw new Error(`failed to seed ${path}: ${response.status()}`);
  }
}

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
