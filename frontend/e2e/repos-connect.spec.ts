import { test, expect } from "@playwright/test";

const TEST_UUID = "11111111-2222-3333-4444-555555555555";

test.describe("GitHub App install redirect flow", () => {
  test("install button triggers same-tab navigation to install URL", async ({
    page,
  }) => {
    await page.route("**/v1/github/install-url", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ url: "https://github.com/apps/test-app/installations/new" }),
      }),
    );
    await page.route("**/v1/auth/me", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          id: "user-1",
          email: "user@example.com",
          email_verified: true,
          current_tenant: { id: "tenant-1", name: "Acme" },
        }),
      }),
    );
    await page.route("**/v1/repos", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ repos: [] }),
      }),
    );

    let assignedUrl = "";
    await page.addInitScript(() => {
      Object.defineProperty(window, "location", {
        writable: true,
        value: {
          ...window.location,
          assign: (url: string) => {
            (window as unknown as Record<string, unknown>).__assignedUrl__ = url;
          },
        },
      });
    });

    await page.goto("/repos");
    await page.getByRole("button", { name: "Connect a repo" }).click();
    await page.getByRole("button", { name: /Install GitHub App/ }).click();
    await page.waitForTimeout(200);

    assignedUrl = await page.evaluate(
      () => (window as unknown as Record<string, unknown>).__assignedUrl__ as string,
    );
    expect(assignedUrl).toContain("github.com/apps");
  });

  test("redirect params auto-open dialog at pick step, show toast, clear URL", async ({
    page,
  }) => {
    await page.route("**/v1/auth/me", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          id: "user-1",
          email: "user@example.com",
          email_verified: true,
          current_tenant: { id: "tenant-1", name: "Acme" },
        }),
      }),
    );
    await page.route("**/v1/repos", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ repos: [] }),
      }),
    );
    await page.route(`**/v1/github/installations/${TEST_UUID}/available-repos**`, (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ total_count: 0, page: 1, per_page: 30, repositories: [] }),
      }),
    );

    await page.goto(
      `/repos?install=success&installation_uuid=${TEST_UUID}&account_login=acme-org`,
    );

    // Dialog should open directly at pick step (no install button visible)
    await expect(page.getByRole("dialog")).toBeVisible();
    await expect(page.locator("#numeric-install-id")).not.toBeVisible();

    // Toast fires
    await expect(page.getByText(/Installed on acme-org/)).toBeVisible();

    // URL params cleaned up
    await expect(page).toHaveURL(/\/repos$/);
  });

  test("pick step submits without numeric input, using UUID from closure", async ({
    page,
  }) => {
    await page.route("**/v1/auth/me", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          id: "user-1",
          email: "user@example.com",
          email_verified: true,
          current_tenant: { id: "tenant-1", name: "Acme" },
        }),
      }),
    );
    await page.route("**/v1/repos", async (route) => {
      if (route.request().method() === "GET") {
        await route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify({ repos: [] }),
        });
      } else {
        const body = route.request().postDataJSON() as Record<string, unknown>;
        expect(body.installation_id).toBe(TEST_UUID);
        await route.fulfill({
          status: 201,
          contentType: "application/json",
          body: JSON.stringify({
            repo_id: "repo-uuid-1",
            full_name: "acme-org/myrepo",
            default_branch: "main",
          }),
        });
      }
    });
    await page.route(`**/v1/github/installations/${TEST_UUID}/available-repos**`, (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          total_count: 1,
          page: 1,
          per_page: 30,
          repositories: [
            {
              id: 42,
              name: "myrepo",
              full_name: "acme-org/myrepo",
              private: false,
              archived: false,
              default_branch: "main",
              html_url: "https://github.com/acme-org/myrepo",
            },
          ],
        }),
      }),
    );

    await page.goto(
      `/repos?install=success&installation_uuid=${TEST_UUID}&account_login=acme-org`,
    );

    await expect(page.getByRole("dialog")).toBeVisible();
    await page.getByRole("button", { name: "acme-org/myrepo" }).click();
    await page.getByRole("button", { name: /Connect repository/ }).click();

    await expect(page.getByText(/Connected acme-org\/myrepo/)).toBeVisible();
  });
});
