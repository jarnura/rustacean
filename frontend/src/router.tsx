import { Suspense } from "react";
import {
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  redirect,
} from "@tanstack/react-router";
import { z } from "zod";
import { AppShell, GlobalSuspenseFallback } from "@/components/AppShell";
import { ForgotPasswordPage } from "@/pages/ForgotPasswordPage";
import { LoginPage } from "@/pages/LoginPage";
import { ReposPlaceholderPage } from "@/pages/ReposPlaceholderPage";
import { ResetPasswordPage } from "@/pages/ResetPasswordPage";
import { SignupPage } from "@/pages/SignupPage";
import { VerifyEmailPage } from "@/pages/VerifyEmailPage";
import { routes } from "@/lib/routes";

const rootRoute = createRootRoute({
  component: () => (
    <AppShell>
      <Suspense fallback={<GlobalSuspenseFallback />}>
        <Outlet />
      </Suspense>
    </AppShell>
  ),
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  beforeLoad: () => {
    throw redirect({ to: routes.login, replace: true });
  },
  component: () => null,
});

const signupRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: routes.signup,
  component: SignupPage,
});

const loginRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: routes.login,
  component: LoginPage,
});

const verifyEmailSearchSchema = z.object({
  token: z.string().optional(),
});

const verifyEmailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: routes.verifyEmail,
  validateSearch: verifyEmailSearchSchema,
  component: VerifyEmailPage,
});

const forgotPasswordRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: routes.forgotPassword,
  component: ForgotPasswordPage,
});

const resetPasswordSearchSchema = z.object({
  token: z.string().optional(),
});

const resetPasswordRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: routes.resetPassword,
  validateSearch: resetPasswordSearchSchema,
  component: ResetPasswordPage,
});

const reposRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: routes.repos,
  component: ReposPlaceholderPage,
});

const catchAllRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "$",
  beforeLoad: () => {
    throw redirect({ to: routes.login, replace: true });
  },
  component: () => null,
});

const routeTree = rootRoute.addChildren([
  indexRoute,
  signupRoute,
  loginRoute,
  verifyEmailRoute,
  forgotPasswordRoute,
  resetPasswordRoute,
  reposRoute,
  catchAllRoute,
]);

export const router = createRouter({
  routeTree,
  defaultPreload: "intent",
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
