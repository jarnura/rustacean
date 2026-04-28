import { expect, test } from "@playwright/test";
import {
  ME_RESPONSE,
  REPOS_RESPONSE,
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
      // Use REPOS_RESPONSE so knownInstallationId is set and dialog opens
      // directly at the pick step (REQ-FE-12 removed the manual install button).
      await page.route("**/v1/repos", (route) => {
        if (route.request().method() === "GET") {
          return route.fulfill({ json: REPOS_RESPONSE });
        }
        return route.continue();
      });
      // Return one repo so the repo-list section renders (and errors show),
      // but the test won't click it — leaving github_repo_id unset.
      await page.route(
        "**/v1/github/installations/*/available-repos**",
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
    });

    test("submitting without github_repo_id shows error alert", async ({
      page,
    }) => {
      await page.goto("/repos");
      await page.waitForLoadState("networkidle");

      // REPOS_RESPONSE has an entry with installation_id so dialog opens at
      // pick step immediately — no install-step button needed.
      await page.getByRole("button", { name: "Connect a repo" }).click();
      await expect(page.getByRole("dialog")).toBeVisible();

      // Submit without selecting any repo — github_repo_id is never set.
      await page.getByRole("button", { name: "Connect repository" }).click();

      // A role="alert" paragraph renders for the github_repo_id Zod error.
      // Scoped to the dialog to avoid matching unrelated alerts on the page.
      const alert = page.getByRole("dialog").locator('[role="alert"]').first();
      await expect(alert).toBeVisible();
      await expect(alert).toContainText(/repository id|required/i);
    });
  });
});
