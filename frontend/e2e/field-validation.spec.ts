import { expect, test } from "@playwright/test";
import {
  ME_RESPONSE,
  REPOS_EMPTY_RESPONSE,
} from "./fixtures/mock-api";
import { LoginPage } from "./pages/LoginPage";

test.describe("Field-level validation errors", () => {
  test.describe("Login form (Field component / aria-invalid)", () => {
    test("empty submit: errors appear under correct fields with aria-invalid", async ({
      page,
    }) => {
      const loginPage = new LoginPage(page);
      await loginPage.goto();

      await loginPage.submit();

      // Both inputs get aria-invalid="true"
      await expect(loginPage.emailInput).toHaveAttribute("aria-invalid", "true");
      await expect(loginPage.passwordInput).toHaveAttribute(
        "aria-invalid",
        "true",
      );

      // Error paragraphs appear with role="alert" under the right inputs
      await expect(loginPage.emailError).toBeVisible();
      await expect(loginPage.emailError).toContainText(/required|valid/i);

      await expect(loginPage.passwordError).toBeVisible();
      await expect(loginPage.passwordError).toContainText(/required/i);

      // aria-describedby links each input to its own error id
      await expect(loginPage.emailInput).toHaveAttribute(
        "aria-describedby",
        "email-error",
      );
      await expect(loginPage.passwordInput).toHaveAttribute(
        "aria-describedby",
        "password-error",
      );
    });

    test("invalid email format: email-specific error text", async ({
      page,
    }) => {
      const loginPage = new LoginPage(page);
      await loginPage.goto();

      await loginPage.emailInput.fill("not-an-email");
      await loginPage.submit();

      await expect(loginPage.emailInput).toHaveAttribute("aria-invalid", "true");
      await expect(loginPage.emailError).toContainText(/valid email/i);

      // Password was not touched — its error fires independently
      await expect(loginPage.passwordInput).toHaveAttribute(
        "aria-invalid",
        "true",
      );
    });

    test("valid email clears aria-invalid after correction", async ({
      page,
    }) => {
      const loginPage = new LoginPage(page);
      await loginPage.goto();

      // Trigger validation
      await loginPage.submit();
      await expect(loginPage.emailInput).toHaveAttribute("aria-invalid", "true");

      // Fix the email — react-hook-form re-validates on change
      await loginPage.emailInput.fill("good@example.com");
      await expect(loginPage.emailInput).toHaveAttribute("aria-invalid", "false");
      await expect(loginPage.emailError).not.toBeVisible();
    });
  });

  test.describe("ConnectRepoDialog pick step (plain input errors)", () => {
    test.beforeEach(async ({ page }) => {
      await page.route("**/v1/me", (route) =>
        route.fulfill({ json: ME_RESPONSE }),
      );
      await page.route("**/v1/repos", (route) => {
        if (route.request().method() === "GET") {
          return route.fulfill({ json: REPOS_EMPTY_RESPONSE });
        }
        return route.continue();
      });
    });

    test("submitting without installation_id shows error alert", async ({
      page,
    }) => {
      await page.goto("/repos");
      await page.waitForLoadState("networkidle");

      await page.getByRole("button", { name: "Connect a repo" }).click();
      // Advance past the install step
      await page
        .getByRole("button", { name: /I've installed the app/ })
        .click();

      // Submit without filling any fields
      await page.getByRole("button", { name: "Connect repository" }).click();

      // A role="alert" paragraph renders under the installation_id input.
      // Scoped to the dialog to avoid matching unrelated alerts on the page.
      const alert = page.getByRole("dialog").locator('[role="alert"]').first();
      await expect(alert).toBeVisible();
      await expect(alert).toContainText(/installation id|positive integer/i);
    });
  });
});
