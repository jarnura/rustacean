import { type Locator, type Page } from "@playwright/test";

export class RepoDetailPage {
  readonly triggerIngestButton: Locator;
  readonly ingestQueuedMessage: Locator;
  readonly repoHeading: Locator;

  constructor(private readonly page: Page) {
    this.triggerIngestButton = page.getByRole("button", {
      name: "Trigger ingestion",
    });
    this.ingestQueuedMessage = page.getByText("Ingestion run queued", {
      exact: true,
    });
    this.repoHeading = page.getByRole("heading", { level: 1 });
  }

  async goto(repoId: string): Promise<void> {
    await this.page.goto(`/repos/${repoId}`);
    await this.page.waitForLoadState("networkidle");
  }

  async triggerIngestion(): Promise<void> {
    await this.triggerIngestButton.click();
  }
}
