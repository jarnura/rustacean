import { expect, test } from "@playwright/test";
import {
  ME_RESPONSE,
  REPOS_RESPONSE,
  REPO_ITEM,
  INGEST_RESPONSE,
} from "./fixtures/mock-api";
import { RepoDetailPage } from "./pages/RepoDetailPage";

// Verifies that useTriggerIngest.onSuccess calls qc.invalidateQueries with the
// repos query key, causing TanStack Query to immediately refetch GET /v1/repos
// while the component is still mounted (connected-repos / ingest-trigger flow).

test.describe("TanStack Query invalidation on mutation success", () => {
  test("useTriggerIngest onSuccess refetches the repos query", async ({
    page,
  }) => {
    let reposGetCount = 0;

    await page.route("**/v1/me", (route) =>
      route.fulfill({ json: ME_RESPONSE }),
    );

    await page.route("**/v1/repos", async (route) => {
      if (route.request().method() === "GET") {
        reposGetCount++;
        await route.fulfill({ json: REPOS_RESPONSE });
      } else {
        await route.continue();
      }
    });

    await page.route("**/v1/repos/*/ingest", (route) =>
      route.fulfill({ json: INGEST_RESPONSE }),
    );

    const repoDetailPage = new RepoDetailPage(page);
    await repoDetailPage.goto(REPO_ITEM.repo_id);

    // At least one GET /v1/repos was made during the initial load.
    const countAfterLoad = reposGetCount;
    expect(countAfterLoad).toBeGreaterThanOrEqual(1);

    // Trigger ingestion — fires POST /v1/repos/{id}/ingest.
    await repoDetailPage.triggerIngestion();

    // The success toast / queued message confirms the mutation resolved.
    await expect(repoDetailPage.ingestQueuedMessage).toBeVisible();

    // Wait for TanStack Query to complete the invalidation refetch.
    await page.waitForLoadState("networkidle");

    // The query key invalidation must have triggered at least one more GET.
    expect(reposGetCount).toBeGreaterThan(countAfterLoad);
  });

  test("useConnectRepo onSuccess refetches the repos query", async ({
    page,
  }) => {
    let reposGetCount = 0;

    await page.route("**/v1/me", (route) =>
      route.fulfill({ json: ME_RESPONSE }),
    );

    await page.route("**/v1/repos", async (route) => {
      if (route.request().method() === "GET") {
        reposGetCount++;
        await route.fulfill({ json: { repos: [] } });
      } else if (route.request().method() === "POST") {
        // Simulate a successful connect
        await route.fulfill({
          json: {
            repo_id: "repo-2",
            full_name: "acme/api",
            installation_id: "install-uuid-1",
            default_branch: "main",
            status: "connected",
            connected_at: "2024-01-01T00:00:00Z",
            connected_by: "user-1",
          },
        });
      } else {
        await route.continue();
      }
    });

    await page.goto("/repos");
    await page.waitForLoadState("networkidle");

    const countAfterLoad = reposGetCount;
    expect(countAfterLoad).toBeGreaterThanOrEqual(1);

    // Open dialog and advance to the pick step.
    await page.getByRole("button", { name: "Connect a repo" }).click();
    await page
      .getByRole("button", { name: /I've installed the app/ })
      .click();

    // Fill in a valid numeric installation ID so Zod accepts the form.
    await page.locator("#numeric-install-id").fill("12345678");

    // Submit — triggers POST /v1/repos which calls onSuccess → invalidateQueries.
    await page.getByRole("button", { name: "Connect repository" }).click();

    await page.waitForLoadState("networkidle");

    // Repos query must have been refetched.
    expect(reposGetCount).toBeGreaterThan(countAfterLoad);
  });
});
