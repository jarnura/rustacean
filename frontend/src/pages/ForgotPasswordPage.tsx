// REQ-FE-02: forgot-password form. The backend always returns 200 (timing-safe
// per ADR-005 #6) so we treat any non-network response as success in the UI.
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { Link } from "react-router-dom";
import { useState } from "react";
import { toast } from "sonner";
import { useForgotPassword } from "@/api";
import { AuthLayout } from "@/components/auth/AuthLayout";
import { Field } from "@/components/ui/Field";
import { SubmitButton } from "@/components/ui/SubmitButton";
import { formatApiError } from "@/lib/errors/api";
import { routes } from "@/lib/routes";
import {
  forgotPasswordFormSchema,
  type ForgotPasswordFormValues,
} from "@/lib/validation/auth";

export function ForgotPasswordPage(): JSX.Element {
  const forgotPassword = useForgotPassword();
  const [submittedEmail, setSubmittedEmail] = useState<string | null>(null);
  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
    reset,
  } = useForm<ForgotPasswordFormValues>({
    resolver: zodResolver(forgotPasswordFormSchema),
    defaultValues: { email: "" },
  });

  const onSubmit = handleSubmit(async (values) => {
    try {
      await forgotPassword.mutateAsync(values);
      setSubmittedEmail(values.email);
      toast.success("If that email exists, a reset link is on its way.");
      reset({ email: "" });
    } catch (error) {
      toast.error(
        formatApiError(error, "We couldn't send the reset email."),
      );
    }
  });

  if (submittedEmail) {
    return (
      <AuthLayout
        title="Check your inbox"
        subtitle={`If an account exists for ${submittedEmail}, we've sent a password reset link.`}
        footer={
          <>
            <Link to={routes.login}>Back to sign in</Link>
            <Link to={routes.resetPassword}>Have a token?</Link>
          </>
        }
      >
        <p className="auth-status auth-status--success">
          Reset email requested.
        </p>
      </AuthLayout>
    );
  }

  return (
    <AuthLayout
      title="Forgot your password?"
      subtitle="Enter your email and we'll send a reset link."
      footer={
        <>
          <Link to={routes.login}>Back to sign in</Link>
          <Link to={routes.signup}>Create an account</Link>
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
        <SubmitButton isLoading={isSubmitting} loadingLabel="Sending…">
          Send reset link
        </SubmitButton>
      </form>
    </AuthLayout>
  );
}
