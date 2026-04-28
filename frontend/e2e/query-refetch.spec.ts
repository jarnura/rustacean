import { expect, test } from "@playwright/test";
import {
  ME_RESPONSE,
  REPOS_RESPONSE,
  REPO_ITEM,
  INGEST_RESPONSE,
  CONNECT_REPO_RESPONSE,
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

    // Listen for the invalidation refetch before triggering the mutation.
    const refetchPromise = page.waitForResponse(
      (r) => {
        const url = new URL(r.url());
        return url.pathname === "/v1/repos" && r.request().method() === "GET";
      },
      { timeout: 10_000 },
    );

    // Trigger ingestion — fires POST /v1/repos/{id}/ingest.
    await repoDetailPage.triggerIngestion();

    // The success message confirms the mutation resolved.
    await expect(repoDetailPage.ingestQueuedMessage).toBeVisible();

    // Wait for TanStack Query to complete the invalidation refetch.
    await refetchPromise;

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
        await route.fulfill({ json: REPOS_RESPONSE });
      } else if (route.request().method() === "POST") {
        await route.fulfill({ json: CONNECT_REPO_RESPONSE });
      } else {
        await route.continue();
      }
    });

    // Provide an available repo so the picker populates and github_repo_id can be set.
    await page.route(
      "**/v1/github/installations/*/available-repos",
      (route) =>
        route.fulfill({
          json: {
            total_count: 1,
            page: 1,
            per_page: 30,
            repositories: [
              {
                id: 99999,
                name: "api",
                full_name: "acme/api",
                private: false,
                archived: false,
                default_branch: "main",
                html_url: "https://github.com/acme/api",
              },
            ],
          },
        }),
    );

    await page.goto("/repos");
    await page.waitForLoadState("networkidle");

    const countAfterLoad = reposGetCount;
    expect(countAfterLoad).toBeGreaterThanOrEqual(1);

    // With REPOS_RESPONSE, knownInstallationId = "install-uuid-1", so the
    // dialog opens directly at the pick step — no install-step button needed.
    await page.getByRole("button", { name: "Connect a repo" }).click();
    await expect(page.getByRole("dialog")).toBeVisible();

    // Wait for the available-repo entry and click it to set github_repo_id.
    await page.getByRole("button", { name: /acme\/api/ }).click();

    // Fill in a valid numeric installation ID so Zod accepts the form.
    await page.locator("#numeric-install-id").fill("12345678");

    // Listen for the invalidation refetch BEFORE submitting so we don't miss it.
    const refetchPromise = page.waitForResponse(
      (r) => {
        const url = new URL(r.url());
        return url.pathname === "/v1/repos" && r.request().method() === "GET";
      },
      { timeout: 10_000 },
    );

    // Submit — triggers POST /v1/repos which calls onSuccess → invalidateQueries.
    await page.getByRole("button", { name: "Connect repository" }).click();

    // Wait for TanStack Query to complete the invalidation refetch.
    await refetchPromise;

    // Repos query must have been refetched.
    expect(reposGetCount).toBeGreaterThan(countAfterLoad);
  });
});
