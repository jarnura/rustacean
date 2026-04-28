import { type Locator, type Page } from "@playwright/test";

export class ReposPage {
  readonly connectButton: Locator;

  constructor(private readonly page: Page) {
    this.connectButton = page.getByRole("button", { name: "Connect a repo" });
  }

  async goto(): Promise<void> {
    await this.page.goto("/repos");
    await this.page.waitForLoadState("networkidle");
  }

  async openConnectDialog(): Promise<ConnectRepoDialog> {
    await this.connectButton.click();
    return new ConnectRepoDialog(this.page);
  }
}

export class ConnectRepoDialog {
  readonly dialog: Locator;
  readonly closeButton: Locator;
  readonly installAppButton: Locator;
  readonly installedButton: Locator;
  readonly installationIdInput: Locator;
  readonly connectButton: Locator;

  constructor(private readonly page: Page) {
    this.dialog = page.getByRole("dialog");
    this.closeButton = page.getByRole("button", { name: "Close" });
    this.installAppButton = page.getByRole("button", {
      name: /Install GitHub App/,
    });
    this.installedButton = page.getByRole("button", {
      name: /I've installed the app/,
    });
    this.installationIdInput = page.locator("#numeric-install-id");
    this.connectButton = page.getByRole("button", {
      name: "Connect repository",
    });
  }

  async dismissWithEscape(): Promise<void> {
    await this.page.keyboard.press("Escape");
  }

  async advanceToPickStep(): Promise<void> {
    await this.installedButton.click();
  }

  getFocusableElements(): Locator {
    return this.dialog.locator(
      "a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex='-1'])",
    );
  }
}
