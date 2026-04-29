import { type Locator, type Page } from "@playwright/test";

export class LoginPage {
  readonly emailInput: Locator;
  readonly passwordInput: Locator;
  readonly submitButton: Locator;
  readonly emailError: Locator;
  readonly passwordError: Locator;

  constructor(private readonly page: Page) {
    this.emailInput = page.locator("#email");
    this.passwordInput = page.locator("#password");
    this.submitButton = page.getByRole("button", { name: "Sign in" });
    this.emailError = page.locator("#email-error");
    this.passwordError = page.locator("#password-error");
  }

  async goto(): Promise<void> {
    await this.page.goto("/login");
  }

  async submit(): Promise<void> {
    await this.submitButton.click();
  }

  async fillAndSubmit(email: string, password: string): Promise<void> {
    await this.emailInput.fill(email);
    await this.passwordInput.fill(password);
    await this.submit();
  }
}
