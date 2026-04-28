import { expect, test } from "@playwright/test";
import {
  mockAuthenticatedSession,
  mockReposList,
  REPOS_EMPTY_RESPONSE,
} from "./fixtures/mock-api";
import { ReposPage } from "./pages/ReposPage";

test.describe("Focus trap in ConnectRepoDialog", () => {
  test.beforeEach(async ({ page }) => {
    await mockAuthenticatedSession(page);
    await mockReposList(page, REPOS_EMPTY_RESPONSE);
  });

  test("Tab key keeps focus inside the open dialog", async ({ page }) => {
    const reposPage = new ReposPage(page);
    await reposPage.goto();

    const dialog = await reposPage.openConnectDialog();
    await expect(dialog.dialog).toBeVisible();

    // Tab through more steps than there are focusable elements — each press
    // must stay within the dialog (cycle, not escape).
    for (let i = 0; i < 9; i++) {
      await page.keyboard.press("Tab");
      const inside = await page.evaluate(() => {
        const d = document.querySelector('[role="dialog"]');
        return d?.contains(document.activeElement) ?? false;
      });
      expect(inside, `Tab press ${String(i + 1)}: focus escaped dialog`).toBe(
        true,
      );
    }
  });

  test("Shift+Tab wraps focus from first to last element", async ({ page }) => {
    const reposPage = new ReposPage(page);
    await reposPage.goto();

    const dialog = await reposPage.openConnectDialog();
    await expect(dialog.dialog).toBeVisible();

    // The dialog auto-focuses the first focusable element on mount.
    // Shift+Tab from first should wrap to the last — still inside the dialog.
    await page.keyboard.press("Shift+Tab");

    const inside = await page.evaluate(() => {
      const d = document.querySelector('[role="dialog"]');
      return d?.contains(document.activeElement) ?? false;
    });
    expect(
      inside,
      "Shift+Tab from first element should wrap inside dialog",
    ).toBe(true);
  });

  test("Tab wraps focus from last to first element", async ({ page }) => {
    const reposPage = new ReposPage(page);
    await reposPage.goto();

    const dialog = await reposPage.openConnectDialog();
    await expect(dialog.dialog).toBeVisible();

    // Count focusable elements, then Tab past the last one to trigger wrap.
    const count = await dialog.getFocusableElements().count();
    for (let i = 0; i < count; i++) {
      await page.keyboard.press("Tab");
    }

    // After count presses focus should have wrapped back inside.
    const inside = await page.evaluate(() => {
      const d = document.querySelector('[role="dialog"]');
      return d?.contains(document.activeElement) ?? false;
    });
    expect(inside, "Tab wrap from last element should land inside dialog").toBe(
      true,
    );
  });

  test("focus restores to trigger button after Escape", async ({ page }) => {
    const reposPage = new ReposPage(page);
    await reposPage.goto();

    // Click the trigger button — this becomes document.activeElement when the
    // dialog captures previousFocusRef on mount.
    const dialog = await reposPage.openConnectDialog();
    await expect(dialog.dialog).toBeVisible();

    await page.keyboard.press("Escape");
    await expect(dialog.dialog).not.toBeVisible();

    // The dialog cleanup effect restores focus to previousFocusRef.
    const focusedText = await page.evaluate(
      () => (document.activeElement as Element | null)?.textContent?.trim() ?? "",
    );
    expect(focusedText).toContain("Connect a repo");
  });
});
