// REQ-FE-02: temporary landing destination after login. The real /repos
// experience and protected-route gating ship with REQ-FE-01 (RUSAA-41).
// This stub exists only so the post-login redirect resolves to a real route.
import { Link } from "react-router-dom";
import { useMe } from "@/api";
import { AuthLayout } from "@/components/auth/AuthLayout";
import { routes } from "@/lib/routes";

export function ReposPlaceholderPage(): JSX.Element {
  const me = useMe({ retry: false, refetchOnWindowFocus: false });

  if (me.isLoading) {
    return (
      <AuthLayout title="Loading…" subtitle="Confirming your session.">
        <p className="auth-status">Checking your session…</p>
      </AuthLayout>
    );
  }

  if (me.isError || !me.data) {
    return (
      <AuthLayout
        title="Please sign in"
        subtitle="You need to be signed in to view your repositories."
        footer={<Link to={routes.login}>Go to sign in</Link>}
      >
        <p className="auth-status auth-status--error">No active session.</p>
      </AuthLayout>
    );
  }

  return (
    <AuthLayout
      title="Repositories"
      subtitle={`Signed in as ${me.data.user.email}.`}
      footer={<Link to={routes.login}>Switch account</Link>}
    >
      <p className="auth-status">
        You're signed in. The full repos experience lands with the application
        shell (REQ-FE-01).
      </p>
    </AuthLayout>
  );
}
