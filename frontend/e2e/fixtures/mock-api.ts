import { type Page } from "@playwright/test";

export const ME_RESPONSE = {
  user: {
    id: "user-1",
    email: "test@example.com",
    email_verified: true,
    created_at: "2024-01-01T00:00:00Z",
    status: "active",
  },
  current_tenant: {
    id: "tenant-1",
    name: "Test Workspace",
    role: "owner",
    slug: "test-workspace",
  },
  available_tenants: [
    {
      id: "tenant-1",
      name: "Test Workspace",
      role: "owner",
      slug: "test-workspace",
    },
  ],
};

export const REPO_ITEM = {
  repo_id: "repo-1",
  full_name: "acme/web-app",
  installation_id: "install-uuid-1",
  default_branch: "main",
  status: "connected",
  connected_at: "2024-01-01T00:00:00Z",
  connected_by: "user-1",
};

export const REPOS_RESPONSE = { repos: [REPO_ITEM] };
export const REPOS_EMPTY_RESPONSE = { repos: [] };

export const INGEST_RESPONSE = { run_id: "run-id-1" };

export const CONNECT_REPO_RESPONSE = {
  repo_id: "repo-2",
  full_name: "acme/api",
  installation_id: "install-uuid-1",
  default_branch: "main",
  status: "connected",
  connected_at: "2024-01-01T00:00:00Z",
  connected_by: "user-1",
};

export const MEMBERS_RESPONSE = {
  members: [
    {
      user_id: "user-1",
      email: "test@example.com",
      role: "owner",
      invited_at: "2024-01-01T00:00:00Z",
    },
  ],
};

export const API_KEYS_RESPONSE = { keys: [] };

export async function mockAuthenticatedSession(page: Page): Promise<void> {
  await page.route("**/v1/me", (route) =>
    route.fulfill({ json: ME_RESPONSE }),
  );
}

export async function mockReposList(
  page: Page,
  response: { repos: typeof REPO_ITEM[] } = REPOS_RESPONSE,
): Promise<void> {
  await page.route("**/v1/repos", (route) => {
    if (route.request().method() === "GET") {
      return route.fulfill({ json: response });
    }
    return route.continue();
  });
}

export async function mockIngestTrigger(page: Page): Promise<void> {
  await page.route("**/v1/repos/*/ingest", (route) =>
    route.fulfill({ json: INGEST_RESPONSE }),
  );
}

export async function mockMembers(page: Page): Promise<void> {
  await page.route("**/v1/tenants/*/members", (route) =>
    route.fulfill({ json: MEMBERS_RESPONSE }),
  );
}
