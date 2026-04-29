import { expect, test } from "@playwright/test";
import {
  mockAuthenticatedSession,
  mockReposList,
  REPOS_EMPTY_RESPONSE,
} from "./fixtures/mock-api";
import { ReposPage } from "./pages/ReposPage";

test.describe("Escape dismissal (WCAG 2.1.2)", () => {
  test.beforeEach(async ({ page }) => {
    await mockAuthenticatedSession(page);
    await mockReposList(page, REPOS_EMPTY_RESPONSE);
  });

  test("ConnectRepoDialog closes on Escape key", async ({ page }) => {
    const reposPage = new ReposPage(page);
    await reposPage.goto();

    const dialog = await reposPage.openConnectDialog();
    await expect(dialog.dialog).toBeVisible();

    await dialog.dismissWithEscape();

    await expect(dialog.dialog).not.toBeVisible();
  });

  test("ConnectRepoDialog closes via ✕ close button", async ({ page }) => {
    const reposPage = new ReposPage(page);
    await reposPage.goto();

    const dialog = await reposPage.openConnectDialog();
    await expect(dialog.dialog).toBeVisible();

    await dialog.closeButton.click();

    await expect(dialog.dialog).not.toBeVisible();
  });

  test("ConnectRepoDialog closes on backdrop click", async ({ page }) => {
    const reposPage = new ReposPage(page);
    await reposPage.goto();

    const dialog = await reposPage.openConnectDialog();
    await expect(dialog.dialog).toBeVisible();

    // The backdrop is the dialog overlay element itself (fixed inset-0).
    // Clicking a corner far outside the inner panel triggers onClose.
    await dialog.dialog.click({ position: { x: 5, y: 5 } });

    await expect(dialog.dialog).not.toBeVisible();
  });

  test("dialog can be opened again after Escape dismissal", async ({
    page,
  }) => {
    const reposPage = new ReposPage(page);
    await reposPage.goto();

    const dialog = await reposPage.openConnectDialog();
    await expect(dialog.dialog).toBeVisible();
    await dialog.dismissWithEscape();
    await expect(dialog.dialog).not.toBeVisible();

    // Re-open
    const dialog2 = await reposPage.openConnectDialog();
    await expect(dialog2.dialog).toBeVisible();
  });
});
