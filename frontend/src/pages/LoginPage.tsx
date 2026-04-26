// REQ-FE-02: login form. On success the server sets the rb_session cookie;
// we route to /repos (Phase 1 exit destination) or /verify-email when the
// account still needs verification.
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { Link, useNavigate } from "@tanstack/react-router";
import { toast } from "sonner";
import { useLogin } from "@/api";
import { AuthLayout } from "@/components/auth/AuthLayout";
import { Field } from "@/components/ui/Field";
import { SubmitButton } from "@/components/ui/SubmitButton";
import { formatApiError } from "@/lib/errors/api";
import { routes } from "@/lib/routes";
import { loginFormSchema, type LoginFormValues } from "@/lib/validation/auth";

export function LoginPage(): JSX.Element {
  const navigate = useNavigate();
  const login = useLogin();
  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<LoginFormValues>({
    resolver: zodResolver(loginFormSchema),
    defaultValues: { email: "", password: "" },
  });

  const onSubmit = handleSubmit(async (values) => {
    try {
      const result = await login.mutateAsync(values);
      if (result.email_verification_required) {
        toast.message("Verify your email to continue.");
        void navigate({ to: routes.verifyEmail, replace: true });
        return;
      }
      toast.success("Signed in.");
      void navigate({ to: routes.repos, replace: true });
    } catch (error) {
      toast.error(
        formatApiError(error, "Login failed. Please try again."),
      );
    }
  });

  return (
    <AuthLayout
      title="Sign in"
      subtitle="Welcome back to Rustacean."
      footer={
        <>
          <Link to={routes.forgotPassword}>Forgot password?</Link>
          <span>
            New here? <Link to={routes.signup}>Create an account</Link>
          </span>
        </>
      }
    >
      <form className="auth-form" onSubmit={onSubmit} noValidate>
        <Field
          label="Email"
          type="email"
          autoComplete="email"
          {...(errors.email?.message ? { error: errors.email.message } : {})}
          {...register("email")}
        />
        <Field
          label="Password"
          type="password"
          autoComplete="current-password"
          {...(errors.password?.message ? { error: errors.password.message } : {})}
          {...register("password")}
        />
        <SubmitButton isLoading={isSubmitting} loadingLabel="Signing in…">
          Sign in
        </SubmitButton>
      </form>
    </AuthLayout>
  );
}
