import { test, expect } from "@playwright/test";

test("app boots and redirects to login", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveURL(/\/login/);
  await expect(page.locator("#root")).toBeVisible();
});
