import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import {
  ME_RESPONSE,
  REPOS_RESPONSE,
  REPOS_EMPTY_RESPONSE,
  REPO_ITEM,
  MEMBERS_RESPONSE,
} from "./fixtures/mock-api";

// Runs @axe-core/playwright against the main routes and fails on any
// serious or critical accessibility violations.

const WCAG_TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"];

async function scanPage(
  page: import("@playwright/test").Page,
): Promise<import("axe-core").Result[]> {
  const results = await new AxeBuilder({ page })
    .withTags(WCAG_TAGS)
    .analyze();
  return results.violations.filter(
    (v) => v.impact === "serious" || v.impact === "critical",
  );
}

test.describe("Axe accessibility scan — main routes", () => {
  test("login page has no serious/critical violations", async ({ page }) => {
    await page.goto("/login");
    await page.waitForLoadState("networkidle");

    const violations = await scanPage(page);
    expect(
      violations,
      violations.map((v) => `${v.id}: ${v.description}`).join("\n"),
    ).toHaveLength(0);
  });

  test("repos list page has no serious/critical violations", async ({
    page,
  }) => {
    await page.route("**/v1/me", (route) =>
      route.fulfill({ json: ME_RESPONSE }),
    );
    await page.route("**/v1/repos", (route) =>
      route.fulfill({ json: REPOS_RESPONSE }),
    );

    await page.goto("/repos");
    await page.waitForLoadState("networkidle");

    const violations = await scanPage(page);
    expect(
      violations,
      violations.map((v) => `${v.id}: ${v.description}`).join("\n"),
    ).toHaveLength(0);
  });

  test("repos list (empty state) has no serious/critical violations", async ({
    page,
  }) => {
    await page.route("**/v1/me", (route) =>
      route.fulfill({ json: ME_RESPONSE }),
    );
    await page.route("**/v1/repos", (route) =>
      route.fulfill({ json: REPOS_EMPTY_RESPONSE }),
    );

    await page.goto("/repos");
    await page.waitForLoadState("networkidle");

    const violations = await scanPage(page);
    expect(
      violations,
      violations.map((v) => `${v.id}: ${v.description}`).join("\n"),
    ).toHaveLength(0);
  });

  test("repo detail page has no serious/critical violations", async ({
    page,
  }) => {
    await page.route("**/v1/me", (route) =>
      route.fulfill({ json: ME_RESPONSE }),
    );
    await page.route("**/v1/repos", (route) =>
      route.fulfill({ json: REPOS_RESPONSE }),
    );

    await page.goto(`/repos/${REPO_ITEM.repo_id}`);
    await page.waitForLoadState("networkidle");

    const violations = await scanPage(page);
    expect(
      violations,
      violations.map((v) => `${v.id}: ${v.description}`).join("\n"),
    ).toHaveLength(0);
  });

  test("connect-repo dialog (ingest trigger surface) has no serious/critical violations", async ({
    page,
  }) => {
    await page.route("**/v1/me", (route) =>
      route.fulfill({ json: ME_RESPONSE }),
    );
    await page.route("**/v1/repos", (route) =>
      route.fulfill({ json: REPOS_EMPTY_RESPONSE }),
    );

    await page.goto("/repos");
    await page.waitForLoadState("networkidle");

    // Open the connect dialog so the modal surface is included in the scan.
    await page.getByRole("button", { name: "Connect a repo" }).click();
    await expect(page.getByRole("dialog")).toBeVisible();

    const violations = await scanPage(page);
    expect(
      violations,
      violations.map((v) => `${v.id}: ${v.description}`).join("\n"),
    ).toHaveLength(0);
  });

  test("members page has no serious/critical violations", async ({ page }) => {
    await page.route("**/v1/me", (route) =>
      route.fulfill({ json: ME_RESPONSE }),
    );
    await page.route("**/v1/tenants/*/members", (route) =>
      route.fulfill({ json: MEMBERS_RESPONSE }),
    );

    await page.goto("/members");
    await page.waitForLoadState("networkidle");

    const violations = await scanPage(page);
    expect(
      violations,
      violations.map((v) => `${v.id}: ${v.description}`).join("\n"),
    ).toHaveLength(0);
  });
});

