// REQ-FE-02: shared layout chrome for unauthenticated auth pages.
import type { ReactNode } from "react";

type AuthLayoutProps = {
  readonly title: string;
  readonly subtitle?: string;
  readonly children: ReactNode;
  readonly footer?: ReactNode;
};

export function AuthLayout({
  title,
  subtitle,
  children,
  footer,
}: AuthLayoutProps): JSX.Element {
  return (
    <main className="auth-shell">
      <section className="auth-card" aria-labelledby="auth-card-title">
        <header className="auth-card__header">
          <h1 id="auth-card-title" className="auth-card__title">
            {title}
          </h1>
          {subtitle ? <p className="auth-card__subtitle">{subtitle}</p> : null}
        </header>
        <div className="auth-card__body">{children}</div>
        {footer ? <footer className="auth-card__footer">{footer}</footer> : null}
      </section>
    </main>
  );
}
