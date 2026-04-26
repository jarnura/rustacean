// REQ-FE-02: routes the auth surfaces. The protected app shell + tenant-aware
// gating arrive with REQ-FE-01 (RUSAA-41); for now the post-login destination
// is a thin placeholder that confirms the session.
import { Navigate, Route, Routes } from "react-router-dom";
import { ForgotPasswordPage } from "@/pages/ForgotPasswordPage";
import { LoginPage } from "@/pages/LoginPage";
import { ReposPlaceholderPage } from "@/pages/ReposPlaceholderPage";
import { ResetPasswordPage } from "@/pages/ResetPasswordPage";
import { SignupPage } from "@/pages/SignupPage";
import { VerifyEmailPage } from "@/pages/VerifyEmailPage";
import { routes } from "@/lib/routes";

export function App(): JSX.Element {
  return (
    <Routes>
      <Route path={routes.signup} element={<SignupPage />} />
      <Route path={routes.login} element={<LoginPage />} />
      <Route path={routes.verifyEmail} element={<VerifyEmailPage />} />
      <Route path={routes.forgotPassword} element={<ForgotPasswordPage />} />
      <Route path={routes.resetPassword} element={<ResetPasswordPage />} />
      <Route path={routes.repos} element={<ReposPlaceholderPage />} />
      <Route path="/" element={<Navigate to={routes.login} replace />} />
      <Route path="*" element={<Navigate to={routes.login} replace />} />
    </Routes>
  );
}
