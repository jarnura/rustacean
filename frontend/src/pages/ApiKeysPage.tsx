// Plaintext key is never re-fetchable from the server, so the create
// response is surfaced in a one-shot panel with a copy action.
import { useMemo, useState } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { Link } from "@tanstack/react-router";
import { toast } from "sonner";
import {
  useApiKeys,
  useCreateApiKey,
  useMe,
  useRevokeApiKey,
} from "@/api";
import { Field } from "@/components/ui/Field";
import { SubmitButton } from "@/components/ui/SubmitButton";
import { formatApiError } from "@/lib/errors/api";
import { routes } from "@/lib/routes";
import {
  createApiKeyFormSchema,
  type ApiKeyScope,
  type CreateApiKeyFormValues,
} from "@/lib/validation/members";

const ALL_SCOPES: ReadonlyArray<ApiKeyScope> = ["read", "write", "admin"];

interface PlaintextKey {
  readonly id: string;
  readonly name: string;
  readonly key: string;
  readonly scopes: ReadonlyArray<ApiKeyScope>;
}

function formatTimestamp(value: string | null | undefined): string {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "—";
  return date.toLocaleString();
}

export function ApiKeysPage(): JSX.Element {
  const me = useMe({ retry: false });

  if (me.isLoading) {
    return (
      <PageContainer>
        <p className="text-sm text-muted-foreground">Loading your session…</p>
      </PageContainer>
    );
  }

  if (me.isError || !me.data) {
    return (
      <PageContainer>
        <h1 className="text-2xl font-semibold tracking-tight">API keys</h1>
        <p className="mt-2 text-sm text-muted-foreground">
          You need to be signed in to manage API keys.
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

  return (
    <ApiKeysPageInner
      tenantId={me.data.current_tenant.id}
      tenantName={me.data.current_tenant.name}
      callerRole={me.data.current_tenant.role}
    />
  );
}

interface ApiKeysPageInnerProps {
  readonly tenantId: string;
  readonly tenantName: string;
  readonly callerRole: string;
}

function ApiKeysPageInner({
  tenantId,
  tenantName,
  callerRole,
}: ApiKeysPageInnerProps): JSX.Element {
  const canManage = callerRole === "owner" || callerRole === "admin";

  const keys = useApiKeys(tenantId);
  const createKey = useCreateApiKey(tenantId);
  const revokeKey = useRevokeApiKey(tenantId);

  const [plaintext, setPlaintext] = useState<PlaintextKey | null>(null);

  const {
    register,
    handleSubmit,
    reset,
    watch,
    formState: { errors, isSubmitting },
  } = useForm<CreateApiKeyFormValues>({
    resolver: zodResolver(createApiKeyFormSchema),
    defaultValues: { name: "", scopes: ["read"] },
  });

  const selectedScopes = watch("scopes");

  const onCreate = handleSubmit(async (values) => {
    try {
      const result = await createKey.mutateAsync({
        name: values.name,
        scopes: values.scopes,
      });
      setPlaintext({
        id: result.id,
        name: result.name,
        key: result.key,
        scopes: result.scopes as ReadonlyArray<ApiKeyScope>,
      });
      reset({ name: "", scopes: ["read"] });
      toast.success(`Created “${result.name}”. Copy the key now.`);
    } catch (error) {
      toast.error(formatApiError(error, "Could not create API key."));
    }
  });

  const sortedKeys = useMemo(() => {
    if (!keys.data) return [];
    return [...keys.data.keys].sort((a, b) => {
      return (
        new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
      );
    });
  }, [keys.data]);

  return (
    <PageContainer>
      <header className="mb-6 flex flex-col gap-1">
        <h1 className="text-2xl font-semibold tracking-tight">API keys</h1>
        <p className="text-sm text-muted-foreground">
          Programmatic access to{" "}
          <span className="font-medium">{tenantName}</span>. Use the{" "}
          <code className="rounded bg-muted px-1 py-0.5 text-xs text-foreground">
            Authorization: Bearer
          </code>{" "}
          header.
        </p>
      </header>

      {plaintext ? (
        <PlaintextKeyPanel
          plaintext={plaintext}
          onDismiss={() => {
            setPlaintext(null);
          }}
        />
      ) : null}

      {canManage ? (
        <section
          aria-labelledby="create-key-heading"
          className="mb-8 rounded-lg border border-border bg-card p-4"
        >
          <h2 id="create-key-heading" className="text-sm font-medium">
            Create an API key
          </h2>
          <p className="mt-1 text-xs text-muted-foreground">
            The plaintext key is shown <strong>only once</strong>. Store it in
            your secret manager immediately — we cannot recover it for you
            later.
          </p>
          <form onSubmit={onCreate} noValidate className="mt-3 flex flex-col gap-4">
            <Field
              label="Name"
              type="text"
              placeholder="ci-deploy"
              {...(errors.name?.message ? { error: errors.name.message } : {})}
              {...register("name")}
            />

            <fieldset className="flex flex-col gap-2">
              <legend className="text-sm font-medium">Scopes</legend>
              <p className="text-xs text-muted-foreground">
                Pick the smallest set of scopes you need.
              </p>
              <div className="flex flex-wrap gap-3">
                {ALL_SCOPES.map((scope) => (
                  <label
                    key={scope}
                    className="flex items-center gap-2 rounded-md border border-input px-3 py-2 text-sm"
                  >
                    <input
                      type="checkbox"
                      value={scope}
                      defaultChecked={scope === "read"}
                      {...register("scopes")}
                    />
                    <span>{scope}</span>
                  </label>
                ))}
              </div>
              {errors.scopes?.message ? (
                <p role="alert" className="text-xs text-destructive">
                  {errors.scopes.message}
                </p>
              ) : null}
              <p className="text-xs text-muted-foreground">
                Selected: {selectedScopes && selectedScopes.length > 0
                  ? selectedScopes.join(", ")
                  : "none"}
              </p>
            </fieldset>

            <div>
              <SubmitButton
                isLoading={isSubmitting || createKey.isPending}
                loadingLabel="Creating…"
              >
                Create key
              </SubmitButton>
            </div>
          </form>
        </section>
      ) : (
        <p className="mb-8 text-sm text-muted-foreground">
          Your role ({callerRole}) cannot create or revoke API keys. Ask an
          admin or owner of {tenantName}.
        </p>
      )}

      <section aria-labelledby="keys-heading">
        <div className="mb-3 flex items-baseline justify-between">
          <h2 id="keys-heading" className="text-sm font-medium">
            Existing keys
          </h2>
          {keys.data ? (
            <span className="text-xs text-muted-foreground">
              {String(keys.data.keys.length)} total
            </span>
          ) : null}
        </div>

        {keys.isLoading ? (
          <p className="text-sm text-muted-foreground">Loading keys…</p>
        ) : keys.isError ? (
          <p className="text-sm text-destructive">
            {formatApiError(keys.error, "Could not load API keys.")}
          </p>
        ) : sortedKeys.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            No API keys yet. Create one above.
          </p>
        ) : (
          <ul className="divide-y divide-border rounded-lg border border-border bg-card">
            {sortedKeys.map((k) => (
              <li
                key={k.id}
                className="flex flex-col gap-3 px-4 py-3 sm:flex-row sm:items-center sm:justify-between"
              >
                <div className="flex flex-col">
                  <span className="text-sm font-medium">{k.name}</span>
                  <span className="text-xs text-muted-foreground">
                    Scopes: {k.scopes.join(", ") || "—"}
                  </span>
                  <span className="text-xs text-muted-foreground">
                    Created {formatTimestamp(k.created_at)}
                    {" · "}
                    Last used {formatTimestamp(k.last_used_at ?? null)}
                  </span>
                </div>
                {canManage ? (
                  <button
                    type="button"
                    disabled={revokeKey.isPending}
                    onClick={async () => {
                      if (
                        !window.confirm(
                          `Revoke API key “${k.name}”? Any service using it will stop working immediately.`,
                        )
                      ) {
                        return;
                      }
                      try {
                        await revokeKey.mutateAsync(k.id);
                        toast.success(`Revoked “${k.name}”.`);
                      } catch (error) {
                        toast.error(
                          formatApiError(error, "Could not revoke key."),
                        );
                      }
                    }}
                    className="rounded-md border border-destructive/40 px-2 py-1 text-xs font-medium text-destructive hover:bg-destructive hover:text-destructive-foreground disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    Revoke
                  </button>
                ) : null}
              </li>
            ))}
          </ul>
        )}
      </section>
    </PageContainer>
  );
}

interface PlaintextKeyPanelProps {
  readonly plaintext: PlaintextKey;
  readonly onDismiss: () => void;
}

function PlaintextKeyPanel({
  plaintext,
  onDismiss,
}: PlaintextKeyPanelProps): JSX.Element {
  const [copied, setCopied] = useState(false);

  const onCopy = async () => {
    try {
      await navigator.clipboard.writeText(plaintext.key);
      setCopied(true);
      toast.success("Copied to clipboard.");
      window.setTimeout(() => {
        setCopied(false);
      }, 2_000);
    } catch {
      toast.error("Copy failed — select the key and copy manually.");
    }
  };

  return (
    <section
      role="alert"
      aria-live="polite"
      aria-labelledby="plaintext-key-heading"
      className="mb-8 rounded-lg border border-amber-500/60 bg-amber-50 p-4 text-amber-950 shadow-sm dark:bg-amber-900/20 dark:text-amber-100"
    >
      <h2 id="plaintext-key-heading" className="text-sm font-semibold">
        New API key — shown only once
      </h2>
      <p className="mt-1 text-xs">
        Copy <strong>{plaintext.name}</strong> now and store it somewhere safe.
        We cannot recover it for you later.
      </p>
      <div className="mt-3 flex flex-col gap-2 sm:flex-row sm:items-center">
        <code className="flex-1 break-all rounded-md border border-amber-500/40 bg-white px-3 py-2 font-mono text-sm text-foreground dark:bg-amber-950/40 dark:text-amber-50">
          {plaintext.key}
        </code>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={onCopy}
            className="rounded-md border border-amber-500/40 bg-amber-500 px-3 py-2 text-xs font-medium text-amber-950 hover:bg-amber-400"
          >
            {copied ? "Copied!" : "Copy key"}
          </button>
          <button
            type="button"
            onClick={onDismiss}
            className="rounded-md border border-amber-500/40 px-3 py-2 text-xs font-medium hover:bg-amber-100 dark:hover:bg-amber-900/40"
          >
            I&apos;ve saved it
          </button>
        </div>
      </div>
      <p className="mt-2 text-xs">
        Scopes: {plaintext.scopes.join(", ") || "—"}
      </p>
    </section>
  );
}

function PageContainer({
  children,
}: {
  readonly children: React.ReactNode;
}): JSX.Element {
  return <div className="container max-w-3xl py-8">{children}</div>;
}
