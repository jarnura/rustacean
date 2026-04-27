import { useEffect, useMemo, useState } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { Link } from "@tanstack/react-router";
import { toast } from "sonner";
import {
  useInviteMember,
  useMe,
  useRemoveMember,
  useTenantMembers,
  useTransferOwnership,
  useUpdateMemberRole,
} from "@/api";
import { Field } from "@/components/ui/Field";
import { SubmitButton } from "@/components/ui/SubmitButton";
import { formatApiError } from "@/lib/errors/api";
import { routes } from "@/lib/routes";
import {
  inviteMemberFormSchema,
  type InviteMemberFormValues,
} from "@/lib/validation/members";

type Role = "owner" | "admin" | "member";

const ASSIGNABLE_ROLES: ReadonlyArray<Exclude<Role, "owner">> = [
  "member",
  "admin",
];

function isManagerRole(role: string | undefined): role is "owner" | "admin" {
  return role === "owner" || role === "admin";
}

function formatInvitedAt(invitedAt: string | null | undefined): string {
  if (!invitedAt) {
    return "—";
  }
  const date = new Date(invitedAt);
  if (Number.isNaN(date.getTime())) {
    return "—";
  }
  return date.toLocaleString();
}

export function MembersPage(): JSX.Element {
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
        <h1 className="text-2xl font-semibold tracking-tight">Members</h1>
        <p className="mt-2 text-sm text-muted-foreground">
          You need to be signed in to manage members.
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
    <MembersPageInner
      tenantId={me.data.current_tenant.id}
      tenantName={me.data.current_tenant.name}
      callerRole={me.data.current_tenant.role as Role}
      callerUserId={me.data.user.id}
    />
  );
}

interface MembersPageInnerProps {
  readonly tenantId: string;
  readonly tenantName: string;
  readonly callerRole: Role;
  readonly callerUserId: string;
}

function MembersPageInner({
  tenantId,
  tenantName,
  callerRole,
  callerUserId,
}: MembersPageInnerProps): JSX.Element {
  const canManage = isManagerRole(callerRole);
  const isOwner = callerRole === "owner";

  const members = useTenantMembers(tenantId);
  const invite = useInviteMember(tenantId);
  const updateRole = useUpdateMemberRole(tenantId);
  const removeMember = useRemoveMember(tenantId);
  const transferOwnership = useTransferOwnership(tenantId);

  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<InviteMemberFormValues>({
    resolver: zodResolver(inviteMemberFormSchema),
    defaultValues: { email: "" },
  });

  const onInvite = handleSubmit(async (values) => {
    try {
      const result = await invite.mutateAsync(values);
      toast.success(
        result.invited
          ? `Invitation sent to ${result.email}.`
          : `${result.email} added to ${tenantName}.`,
      );
      reset({ email: "" });
    } catch (error) {
      toast.error(formatApiError(error, "Could not send invite."));
    }
  });

  const sortedMembers = useMemo(() => {
    if (!members.data) {
      return [];
    }
    const roleOrder: Record<string, number> = {
      owner: 0,
      admin: 1,
      member: 2,
    };
    return [...members.data.members].sort((a, b) => {
      const ra = roleOrder[a.role] ?? 99;
      const rb = roleOrder[b.role] ?? 99;
      if (ra !== rb) return ra - rb;
      return a.email.localeCompare(b.email);
    });
  }, [members.data]);

  return (
    <PageContainer>
      <header className="mb-6 flex flex-col gap-1">
        <h1 className="text-2xl font-semibold tracking-tight">Members</h1>
        <p className="text-sm text-muted-foreground">
          Manage who can access <span className="font-medium">{tenantName}</span>.
        </p>
      </header>

      {canManage ? (
        <section
          aria-labelledby="invite-heading"
          className="mb-8 rounded-lg border border-border bg-card p-4"
        >
          <h2 id="invite-heading" className="text-sm font-medium">
            Invite a member
          </h2>
          <p className="mt-1 text-xs text-muted-foreground">
            We&apos;ll send an invitation email if they don&apos;t have an account
            yet, or add them directly if they do.
          </p>
          <form
            onSubmit={onInvite}
            noValidate
            className="mt-3 flex flex-col gap-3 sm:flex-row sm:items-end"
          >
            <div className="flex-1">
              <Field
                label="Email"
                type="email"
                autoComplete="email"
                placeholder="teammate@example.com"
                {...(errors.email?.message
                  ? { error: errors.email.message }
                  : {})}
                {...register("email")}
              />
            </div>
            <SubmitButton
              isLoading={isSubmitting || invite.isPending}
              loadingLabel="Sending…"
            >
              Send invite
            </SubmitButton>
          </form>
        </section>
      ) : null}

      <section aria-labelledby="members-heading">
        <div className="mb-3 flex items-baseline justify-between">
          <h2 id="members-heading" className="text-sm font-medium">
            All members
          </h2>
          {members.data ? (
            <span className="text-xs text-muted-foreground">
              {String(members.data.members.length)} total
            </span>
          ) : null}
        </div>

        {members.isLoading ? (
          <p className="text-sm text-muted-foreground">Loading members…</p>
        ) : members.isError ? (
          <p className="text-sm text-destructive">
            {formatApiError(members.error, "Could not load members.")}
          </p>
        ) : sortedMembers.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            No members yet — invite someone above.
          </p>
        ) : (
          <ul className="divide-y divide-border rounded-lg border border-border bg-card">
            {sortedMembers.map((m) => {
              const isSelf = m.user_id === callerUserId;
              return (
                <MemberRow
                  key={m.user_id}
                  email={m.email}
                  role={m.role as Role}
                  invitedAt={m.invited_at ?? null}
                  userId={m.user_id}
                  isSelf={isSelf}
                  canManage={canManage}
                  isOwner={isOwner}
                  onChangeRole={async (nextRole) => {
                    try {
                      await updateRole.mutateAsync({
                        uid: m.user_id,
                        body: { role: nextRole },
                      });
                      toast.success(`Updated ${m.email} to ${nextRole}.`);
                    } catch (error) {
                      toast.error(
                        formatApiError(error, "Could not update role."),
                      );
                    }
                  }}
                  onRemove={async () => {
                    if (
                      !window.confirm(
                        `Remove ${m.email} from ${tenantName}? They will lose access immediately.`,
                      )
                    ) {
                      return;
                    }
                    try {
                      await removeMember.mutateAsync(m.user_id);
                      toast.success(`${m.email} removed.`);
                    } catch (error) {
                      toast.error(
                        formatApiError(error, "Could not remove member."),
                      );
                    }
                  }}
                  onTransferOwnership={async () => {
                    if (
                      !window.confirm(
                        `Transfer ownership of ${tenantName} to ${m.email}? You will become an admin.`,
                      )
                    ) {
                      return;
                    }
                    try {
                      await transferOwnership.mutateAsync({
                        user_id: m.user_id,
                      });
                      toast.success(`${m.email} is now the owner.`);
                    } catch (error) {
                      toast.error(
                        formatApiError(
                          error,
                          "Could not transfer ownership.",
                        ),
                      );
                    }
                  }}
                />
              );
            })}
          </ul>
        )}
      </section>
    </PageContainer>
  );
}

interface MemberRowProps {
  readonly email: string;
  readonly role: Role;
  readonly invitedAt: string | null;
  readonly userId: string;
  readonly isSelf: boolean;
  readonly canManage: boolean;
  readonly isOwner: boolean;
  readonly onChangeRole: (nextRole: Exclude<Role, "owner">) => Promise<void>;
  readonly onRemove: () => Promise<void>;
  readonly onTransferOwnership: () => Promise<void>;
}

function MemberRow({
  email,
  role,
  invitedAt,
  isSelf,
  canManage,
  isOwner,
  onChangeRole,
  onRemove,
  onTransferOwnership,
}: MemberRowProps): JSX.Element {
  const [pendingRole, setPendingRole] = useState<Role>(role);
  const [busy, setBusy] = useState(false);

  // Keep the local select in sync with refetched data.
  useEffect(() => {
    setPendingRole(role);
  }, [role]);

  const isOwnerRow = role === "owner";
  // Admins cannot manage other admins or the owner; only the owner can.
  const canActOnRow =
    canManage &&
    !isOwnerRow &&
    !isSelf &&
    (isOwner || role !== "admin");

  const canTransferOwnership = isOwner && !isOwnerRow;

  return (
    <li className="flex flex-col gap-3 px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
      <div className="flex flex-col">
        <span className="text-sm font-medium">
          {email}
          {isSelf ? (
            <span className="ml-2 text-xs text-muted-foreground">(you)</span>
          ) : null}
        </span>
        <span className="text-xs text-muted-foreground">
          Joined {formatInvitedAt(invitedAt)}
        </span>
      </div>

      <div className="flex items-center gap-2">
        {isOwnerRow ? (
          <span className="rounded-md border border-border bg-secondary px-2 py-1 text-xs font-medium text-secondary-foreground">
            owner
          </span>
        ) : canActOnRow ? (
          <label className="flex items-center gap-2 text-xs text-muted-foreground">
            <span className="sr-only">Role for {email}</span>
            <select
              value={pendingRole}
              disabled={busy}
              onChange={async (event) => {
                const next = event.target.value as Exclude<Role, "owner">;
                setPendingRole(next);
                if (next === role) {
                  return;
                }
                setBusy(true);
                try {
                  await onChangeRole(next);
                } finally {
                  setBusy(false);
                }
              }}
              className="rounded-md border border-input bg-background px-2 py-1 text-sm text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              {ASSIGNABLE_ROLES.map((r) => (
                <option key={r} value={r}>
                  {r}
                </option>
              ))}
            </select>
          </label>
        ) : (
          <span className="rounded-md border border-border bg-muted px-2 py-1 text-xs font-medium text-muted-foreground">
            {role}
          </span>
        )}

        {canTransferOwnership ? (
          <button
            type="button"
            disabled={busy}
            onClick={async () => {
              setBusy(true);
              try {
                await onTransferOwnership();
              } finally {
                setBusy(false);
              }
            }}
            className="rounded-md border border-border px-2 py-1 text-xs text-foreground hover:bg-accent hover:text-accent-foreground disabled:cursor-not-allowed disabled:opacity-50"
          >
            Make owner
          </button>
        ) : null}

        {canActOnRow ? (
          <button
            type="button"
            disabled={busy}
            onClick={async () => {
              setBusy(true);
              try {
                await onRemove();
              } finally {
                setBusy(false);
              }
            }}
            className="rounded-md border border-destructive/40 px-2 py-1 text-xs font-medium text-destructive hover:bg-destructive hover:text-destructive-foreground disabled:cursor-not-allowed disabled:opacity-50"
          >
            Remove
          </button>
        ) : null}
      </div>
    </li>
  );
}

function PageContainer({
  children,
}: {
  readonly children: React.ReactNode;
}): JSX.Element {
  return (
    <div className="container max-w-3xl py-8">
      {children}
    </div>
  );
}
