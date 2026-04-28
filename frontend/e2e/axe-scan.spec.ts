import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import {
  REPOS_RESPONSE,
  REPOS_EMPTY_RESPONSE,
  REPO_ITEM,
  MEMBERS_RESPONSE,
  mockAuthenticatedSession,
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

type MockFn = (page: import("@playwright/test").Page) => Promise<void>;

const ROUTE_MOCKS: Partial<Record<string, MockFn>> = {
  "/repos": async (page) => {
    await mockAuthenticatedSession(page);
    await page.route("**/v1/repos", (r) => r.fulfill({ json: REPOS_RESPONSE }));
  },
  "/repos-empty": async (page) => {
    await mockAuthenticatedSession(page);
    await page.route("**/v1/repos", (r) =>
      r.fulfill({ json: REPOS_EMPTY_RESPONSE }),
    );
  },
  [`/repos/${REPO_ITEM.repo_id}`]: async (page) => {
    await mockAuthenticatedSession(page);
    await page.route("**/v1/repos", (r) => r.fulfill({ json: REPOS_RESPONSE }));
  },
  "/members": async (page) => {
    await mockAuthenticatedSession(page);
    await page.route("**/v1/tenants/*/members", (r) =>
      r.fulfill({ json: MEMBERS_RESPONSE }),
    );
  },
};

test.describe("Axe accessibility scan — main routes", () => {
  test("login page has no serious/critical violations", async ({ page }) => {
    await page.goto("/login");
    await expect(page.getByRole("heading")).toBeVisible();

    const violations = await scanPage(page);
    expect(
      violations,
      violations.map((v) => `${v.id}: ${v.description}`).join("\n"),
    ).toHaveLength(0);
  });

  test("repos list page has no serious/critical violations", async ({
    page,
  }) => {
    const mock = ROUTE_MOCKS["/repos"];
    if (mock) await mock(page);

    await page.goto("/repos");
    await expect(
      page.getByRole("heading", { name: "Repositories" }),
    ).toBeVisible();

    const violations = await scanPage(page);
    expect(
      violations,
      violations.map((v) => `${v.id}: ${v.description}`).join("\n"),
    ).toHaveLength(0);
  });

  test("repos list (empty state) has no serious/critical violations", async ({
    page,
  }) => {
    const mock = ROUTE_MOCKS["/repos-empty"];
    if (mock) await mock(page);

    await page.goto("/repos");
    await expect(
      page.getByRole("heading", { name: "Repositories" }),
    ).toBeVisible();

    const violations = await scanPage(page);
    expect(
      violations,
      violations.map((v) => `${v.id}: ${v.description}`).join("\n"),
    ).toHaveLength(0);
  });

  test("repo detail page has no serious/critical violations", async ({
    page,
  }) => {
    const mock = ROUTE_MOCKS[`/repos/${REPO_ITEM.repo_id}`];
    if (mock) await mock(page);

    await page.goto(`/repos/${REPO_ITEM.repo_id}`);
    await expect(page.getByRole("heading", { level: 1 })).toBeVisible();

    const violations = await scanPage(page);
    expect(
      violations,
      violations.map((v) => `${v.id}: ${v.description}`).join("\n"),
    ).toHaveLength(0);
  });

  test("connect-repo dialog (ingest trigger surface) has no serious/critical violations", async ({
    page,
  }) => {
    const mock = ROUTE_MOCKS["/repos-empty"];
    if (mock) await mock(page);

    await page.goto("/repos");
    await expect(
      page.getByRole("heading", { name: "Repositories" }),
    ).toBeVisible();

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
    const mock = ROUTE_MOCKS["/members"];
    if (mock) await mock(page);

    await page.goto("/members");
    await expect(
      page.getByRole("heading", { name: "Members" }),
    ).toBeVisible();

    const violations = await scanPage(page);
    expect(
      violations,
      violations.map((v) => `${v.id}: ${v.description}`).join("\n"),
    ).toHaveLength(0);
  });
});
