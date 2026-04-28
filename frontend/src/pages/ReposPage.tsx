import { useState } from "react";
import { Link } from "@tanstack/react-router";
import { toast } from "sonner";
import {
  useMe,
  useRepos,
  useConnectRepo,
  useAvailableRepos,
  useGithubInstallUrl,
  type RepoItem,
  type AvailableRepo,
} from "@/api";
import { formatApiError } from "@/lib/errors/api";
import { routes } from "@/lib/routes";

// ---------------------------------------------------------------------------
// ReposPage — entry point
// ---------------------------------------------------------------------------

export function ReposPage(): JSX.Element {
  const me = useMe({ retry: false });

  if (me.isLoading) {
    return (
      <PageContainer>
        <p className="text-sm text-muted-foreground">Loading session…</p>
      </PageContainer>
    );
  }

  if (me.isError || !me.data) {
    return (
      <PageContainer>
        <h1 className="text-2xl font-semibold tracking-tight">Repositories</h1>
        <p className="mt-2 text-sm text-muted-foreground">
          You need to be signed in to manage repositories.
        </p>
        <Link
          to={routes.login}
          className="mt-4 inline-block text-sm text-primary hover:underline"
        >
          Sign in →
        </Link>
      </PageContainer>
    );
  }

  return <ReposPageInner tenantId={me.data.current_tenant.id} />;
}

// ---------------------------------------------------------------------------
// Inner page (tenant resolved)
// ---------------------------------------------------------------------------

interface ReposPageInnerProps {
  readonly tenantId: string;
}

function ReposPageInner({ tenantId }: ReposPageInnerProps): JSX.Element {
  const repos = useRepos(tenantId);
  const [showConnect, setShowConnect] = useState(false);

  const connectedList: readonly RepoItem[] = repos.data?.repos ?? [];

  // Derive the installation UUID from the first connected repo (if any).
  // This lets us populate the available-repos picker for subsequent connects.
  const knownInstallationId = connectedList[0]?.installation_id ?? null;

  return (
    <PageContainer>
      <header className="mb-6 flex items-start justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Repositories</h1>
          <p className="mt-1 text-sm text-muted-foreground">
            Connected GitHub repositories for this workspace.
          </p>
        </div>
        <button
          type="button"
          onClick={() => setShowConnect(true)}
          className="shrink-0 rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground shadow-sm hover:bg-primary/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          Connect a repo
        </button>
      </header>

      {repos.isLoading ? (
        <p className="text-sm text-muted-foreground">Loading repositories…</p>
      ) : repos.isError ? (
        <p className="text-sm text-destructive">
          {formatApiError(repos.error, "Could not load repositories.")}
        </p>
      ) : connectedList.length === 0 ? (
        <EmptyState onConnect={() => setShowConnect(true)} />
      ) : (
        <RepoList repos={connectedList} />
      )}

      {showConnect ? (
        <ConnectRepoDialog
          tenantId={tenantId}
          installationId={knownInstallationId}
          onClose={() => setShowConnect(false)}
          onSuccess={() => setShowConnect(false)}
        />
      ) : null}
    </PageContainer>
  );
}

// ---------------------------------------------------------------------------
// Repo list
// ---------------------------------------------------------------------------

function RepoList({ repos }: { readonly repos: readonly RepoItem[] }): JSX.Element {
  return (
    <ul className="divide-y divide-border rounded-lg border border-border bg-card">
      {repos.map((repo) => (
        <RepoRow key={repo.repo_id} repo={repo} />
      ))}
    </ul>
  );
}

function RepoRow({ repo }: { readonly repo: RepoItem }): JSX.Element {
  const connectedAt = new Date(repo.connected_at);
  const relativeDate = Number.isNaN(connectedAt.getTime())
    ? "—"
    : connectedAt.toLocaleDateString();

  return (
    <li className="flex flex-col gap-2 px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
      <div className="flex flex-col gap-0.5">
        <span className="text-sm font-medium">{repo.full_name}</span>
        <span className="text-xs text-muted-foreground">
          Branch: <span className="font-mono">{repo.default_branch}</span>
          {" · "}
          Connected {relativeDate}
        </span>
      </div>
      <div className="flex items-center gap-2">
        <StatusBadge status={repo.status} />
        <Link
          to="/repos/$repoId"
          params={{ repoId: repo.repo_id }}
          className="rounded-md border border-border px-2 py-1 text-xs font-medium text-foreground hover:bg-accent hover:text-accent-foreground"
        >
          View
        </Link>
      </div>
    </li>
  );
}

function StatusBadge({ status }: { readonly status: string }): JSX.Element {
  const colors =
    status === "connected"
      ? "border-green-500/30 bg-green-50 text-green-700 dark:bg-green-950 dark:text-green-300"
      : status === "ingesting"
        ? "border-blue-500/30 bg-blue-50 text-blue-700 dark:bg-blue-950 dark:text-blue-300"
        : "border-border bg-muted text-muted-foreground";
  return (
    <span className={`rounded-md border px-2 py-0.5 text-xs font-medium ${colors}`}>
      {status}
    </span>
  );
}

// ---------------------------------------------------------------------------
// Empty state
// ---------------------------------------------------------------------------

function EmptyState({ onConnect }: { readonly onConnect: () => void }): JSX.Element {
  return (
    <div className="flex flex-col items-center rounded-lg border border-dashed border-border py-12 text-center">
      <p className="text-sm font-medium text-foreground">No repositories connected</p>
      <p className="mt-1 text-xs text-muted-foreground">
        Connect a GitHub repository to start indexing your codebase.
      </p>
      <button
        type="button"
        onClick={onConnect}
        className="mt-4 rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:bg-primary/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        Connect your first repo
      </button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Connect repo dialog
// ---------------------------------------------------------------------------

type ConnectStep = "install" | "pick";

interface ConnectRepoDialogProps {
  readonly tenantId: string;
  readonly installationId: string | null;
  readonly onClose: () => void;
  readonly onSuccess: () => void;
}

function ConnectRepoDialog({
  tenantId,
  installationId,
  onClose,
  onSuccess,
}: ConnectRepoDialogProps): JSX.Element {
  const [step, setStep] = useState<ConnectStep>(
    installationId ? "pick" : "install",
  );
  const [resolvedInstallId, setResolvedInstallId] = useState<string>(
    installationId ?? "",
  );
  // Numeric installation_id (GitHub's i64) required by POST /v1/repos.
  // Not available from connected-repos list (which only stores internal UUID).
  // User must supply it; they can find it in the GitHub App callback response.
  const [numericInstallId, setNumericInstallId] = useState("");

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="connect-repo-title"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
      onClick={(e) => {
        if (e.target === e.currentTarget) {
          onClose();
        }
      }}
    >
      <div className="flex w-full max-w-lg flex-col gap-4 rounded-lg border border-border bg-background p-6 shadow-xl">
        <div className="flex items-start justify-between">
          <h2
            id="connect-repo-title"
            className="text-lg font-semibold tracking-tight"
          >
            Connect a repository
          </h2>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close"
            className="rounded-md p-1 text-muted-foreground hover:bg-accent hover:text-accent-foreground"
          >
            ✕
          </button>
        </div>

        {step === "install" ? (
          <InstallStep
            onInstalled={(installUuid) => {
              setResolvedInstallId(installUuid);
              setStep("pick");
            }}
          />
        ) : (
          <PickRepoStep
            tenantId={tenantId}
            installationUuid={resolvedInstallId}
            numericInstallId={numericInstallId}
            onNumericInstallIdChange={setNumericInstallId}
            onSuccess={onSuccess}
          />
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Step 1 — Install GitHub App
// ---------------------------------------------------------------------------

function InstallStep({
  onInstalled,
}: {
  readonly onInstalled: (installUuid: string) => void;
}): JSX.Element {
  const installUrl = useGithubInstallUrl();

  const handleInstall = async () => {
    try {
      const result = await installUrl.mutateAsync();
      window.open(result.url, "_blank", "noopener");
    } catch (err) {
      toast.error(formatApiError(err, "Could not generate install link."));
    }
  };

  return (
    <div className="flex flex-col gap-4">
      <p className="text-sm text-muted-foreground">
        First, install the GitHub App on the organization or account that owns
        the repositories you want to connect.
      </p>
      <button
        type="button"
        disabled={installUrl.isPending}
        onClick={handleInstall}
        className="rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-60"
      >
        {installUrl.isPending ? "Generating link…" : "Install GitHub App →"}
      </button>
      <p className="text-xs text-muted-foreground">
        After installing, return here and click{" "}
        <strong>I&apos;ve installed the app</strong>.
      </p>
      <button
        type="button"
        onClick={() => onInstalled("")}
        className="self-start text-sm font-medium text-primary hover:underline"
      >
        I&apos;ve installed the app →
      </button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Step 2 — Pick repo from available list
// ---------------------------------------------------------------------------

interface PickRepoStepProps {
  readonly tenantId: string;
  readonly installationUuid: string;
  readonly numericInstallId: string;
  readonly onNumericInstallIdChange: (v: string) => void;
  readonly onSuccess: () => void;
}

function PickRepoStep({
  tenantId,
  installationUuid,
  numericInstallId,
  onNumericInstallIdChange,
  onSuccess,
}: PickRepoStepProps): JSX.Element {
  const available = useAvailableRepos(installationUuid, 1, {
    enabled: installationUuid.length > 0,
  });
  const connect = useConnectRepo(tenantId);
  const [selected, setSelected] = useState<AvailableRepo | null>(null);
  const [busy, setBusy] = useState(false);

  const numericId = Number(numericInstallId);
  const canConnect =
    selected !== null && Number.isFinite(numericId) && numericId > 0;

  const handleConnect = async () => {
    if (!selected || !canConnect) {
      return;
    }
    setBusy(true);
    try {
      const result = await connect.mutateAsync({
        installation_id: numericId,
        github_repo_id: selected.id,
        default_branch: selected.default_branch || null,
      });
      toast.success(`Connected ${result.full_name}.`);
      onSuccess();
    } catch (err) {
      toast.error(formatApiError(err, "Could not connect repository."));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col gap-4">
      {/* Numeric installation ID input */}
      <div className="flex flex-col gap-1">
        <label
          htmlFor="numeric-install-id"
          className="text-xs font-medium text-foreground"
        >
          GitHub installation ID (numeric)
        </label>
        <input
          id="numeric-install-id"
          type="number"
          min={1}
          placeholder="e.g. 12345678"
          value={numericInstallId}
          onChange={(e) => onNumericInstallIdChange(e.target.value)}
          className="rounded-md border border-input bg-background px-3 py-1.5 text-sm text-foreground placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        />
        <p className="text-xs text-muted-foreground">
          Find this in the GitHub App callback response (
          <code className="rounded bg-muted px-1 text-xs">installation_id</code> field)
          or your GitHub App settings.
        </p>
      </div>

      {/* Available repos list */}
      {installationUuid.length === 0 ? (
        <p className="rounded-md bg-muted px-3 py-2 text-xs text-muted-foreground">
          Return here after installing the GitHub App to see available repositories.
        </p>
      ) : available.isLoading ? (
        <p className="text-sm text-muted-foreground">Loading available repos…</p>
      ) : available.isError ? (
        <p className="text-sm text-destructive">
          {formatApiError(available.error, "Could not load repositories.")}
        </p>
      ) : !available.data || available.data.repositories.length === 0 ? (
        <p className="text-sm text-muted-foreground">No repositories accessible.</p>
      ) : (
        <div className="flex flex-col gap-1">
          <p className="text-xs font-medium text-foreground">
            Select a repository
          </p>
          <ul className="max-h-48 divide-y divide-border overflow-y-auto rounded-md border border-border bg-card">
            {available.data.repositories.map((repo) => (
              <li key={repo.id}>
                <button
                  type="button"
                  onClick={() => setSelected(repo)}
                  className={`w-full px-3 py-2 text-left text-sm hover:bg-accent hover:text-accent-foreground ${
                    selected?.id === repo.id
                      ? "bg-primary/10 font-medium"
                      : ""
                  }`}
                >
                  <span className="block font-medium">{repo.full_name}</span>
                  <span className="block text-xs text-muted-foreground">
                    {repo.private ? "Private" : "Public"}
                    {" · "}
                    {repo.default_branch}
                    {repo.archived ? " · archived" : ""}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}

      <button
        type="button"
        disabled={!canConnect || busy}
        onClick={handleConnect}
        className="rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-60"
      >
        {busy ? "Connecting…" : "Connect repository"}
      </button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Layout helper
// ---------------------------------------------------------------------------

function PageContainer({
  children,
}: {
  readonly children: React.ReactNode;
}): JSX.Element {
  return <div className="container max-w-3xl py-8">{children}</div>;
}
