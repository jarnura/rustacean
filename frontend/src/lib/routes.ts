// REQ-FE-02: shared route paths so navigation calls stay symbolic.
export const routes = {
  signup: "/signup",
  login: "/login",
  verifyEmail: "/verify-email",
  forgotPassword: "/forgot-password",
  resetPassword: "/reset-password",
  repos: "/repos",
} as const;

export type RoutePath = (typeof routes)[keyof typeof routes];
