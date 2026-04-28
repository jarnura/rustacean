import { test, expect } from "@playwright/test";

// Must be a valid RFC 4122 UUID (Zod v4 z.uuid() enforces version+variant bits)
const TEST_UUID = "12345678-1234-4321-89ab-1234567890ab";

// Shared mock response shapes matching the generated API schema
const ME_RESPONSE = {
  user: {
    id: "user-uuid-1",
    email: "user@example.com",
    email_verified: true,
    status: "active",
    created_at: "2024-01-01T00:00:00Z",
  },
  current_tenant: { id: "tenant-uuid-1", name: "Acme", role: "owner", slug: "acme" },
  available_tenants: [
    { id: "tenant-uuid-1", name: "Acme", role: "owner", slug: "acme" },
  ],
};

async function mockBase(page: import("@playwright/test").Page) {
  await page.route("**/v1/me", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(ME_RESPONSE),
    }),
  );
  await page.route("**/v1/repos", (route) => {
    if (route.request().method() === "GET") {
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ repos: [] }),
      });
    }
    return route.continue();
  });
}

test.describe("GitHub App install redirect flow", () => {
  test("install button triggers same-tab navigation to install URL", async ({
    page,
  }) => {
    // The install-url mock returns a same-origin sentinel so we can assert
    // window.location.assign fired (not window.open) via page.waitForURL.
    const sentinelUrl = "http://localhost:4173/__install_sentinel__";
    await mockBase(page);
    await page.route("**/v1/github/install-url", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ url: sentinelUrl, state_token: "test-tok" }),
      }),
    );
    // Absorb the sentinel so it doesn't 404-crash
    await page.route("**/__install_sentinel__**", (route) =>
      route.fulfill({ status: 200, contentType: "text/html", body: "<html/>" }),
    );

    await page.goto("/repos");
    await page.getByRole("button", { name: "Connect a repo" }).click();
    await page.getByRole("button", { name: /Install GitHub App/ }).click();

    // window.location.assign navigates in the same tab — page URL changes.
    // window.open would NOT change the page URL.
    await page.waitForURL("**/__install_sentinel__**");
    expect(page.url()).toContain("__install_sentinel__");
  });

  test("redirect params auto-open dialog at pick step, show toast, clear URL", async ({
    page,
  }) => {
    await mockBase(page);
    await page.route(
      `**/v1/github/installations/${TEST_UUID}/available-repos**`,
      (route) =>
        route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify({
            total_count: 0,
            page: 1,
            per_page: 30,
            repositories: [],
          }),
        }),
    );

    await page.goto(
      `/repos?install=success&installation_uuid=${TEST_UUID}&account_login=acme-org`,
    );

    // Dialog auto-opens at pick step — role="dialog" is set explicitly on the overlay div
    await expect(page.getByRole("dialog")).toBeVisible();

    // No numeric installation ID input present
    await expect(page.locator("#numeric-install-id")).not.toBeAttached();

    // Toast fires with account name
    await expect(page.getByText(/Installed on acme-org/)).toBeVisible();

    // URL cleaned up — no install params remain
    await expect(page).toHaveURL(/\/repos$/);
  });

  test("pick step submits without numeric input, using UUID from closure", async ({
    page,
  }) => {
    let connectBody: Record<string, unknown> = {};

    await mockBase(page);
    await page.route("**/v1/repos", async (route) => {
      if (route.request().method() === "POST") {
        connectBody = route.request().postDataJSON() as Record<string, unknown>;
        await route.fulfill({
          status: 201,
          contentType: "application/json",
          body: JSON.stringify({
            repo_id: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
            full_name: "acme-org/myrepo",
            default_branch: "main",
          }),
        });
      } else {
        await route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify({ repos: [] }),
        });
      }
    });
    await page.route(
      `**/v1/github/installations/${TEST_UUID}/available-repos**`,
      (route) =>
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
    // Select the repo from the list
    await page.getByRole("button", { name: /acme-org\/myrepo/ }).click();
    await page.getByRole("button", { name: /Connect repository/ }).click();

    await expect(page.getByText(/Connected acme-org\/myrepo/)).toBeVisible();

    // Verify the POST body used the UUID from the URL (not a numeric field)
    expect(connectBody.installation_id).toBe(TEST_UUID);
    expect(typeof connectBody.installation_id).toBe("string");
  });
});
