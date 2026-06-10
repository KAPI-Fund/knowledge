import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test } from "@playwright/test";

const fixturesDir = join(dirname(fileURLToPath(import.meta.url)), "..", "fixtures", "uploads");

test("admin can configure source watch and scan external files into project sources", async ({
  page,
}) => {
  await signInAsAdmin(page);

  await createProject(page, "source-watch-project");
  await expect(page.getByRole("heading", { name: "source-watch-project" })).toBeVisible();

  await page.getByRole("link", { name: "Source Watch" }).click();
  await page.getByLabel("Watch Path").fill(fixturesDir);
  await page.getByRole("button", { name: "Save Source Watch" }).click();
  await expect(page.getByText("Saved source watch settings.")).toBeVisible();

  await page.getByRole("button", { name: "Scan Now" }).click();
  await expect(
    page.getByText(/Scanned \d+ file\(s\), copied \d+, queued \d+ ingest task\(s\)\./),
  ).toBeVisible();

  await page.getByRole("link", { name: "Sources" }).click();
  await expect(page.getByText("raw/sources/attention.md").first()).toBeVisible();
  await expect(page.getByText("raw/sources/team-a/child.md").first()).toBeVisible();
  await expect(page.getByText("raw/sources/team-a/docs/peer.md").first()).toBeVisible();

  await page.getByRole("link", { name: "Tasks" }).click();
  await expect(
    page.getByRole("listitem").filter({ hasText: "Ingest raw/sources/attention.md" }),
  ).toContainText("succeeded");
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
