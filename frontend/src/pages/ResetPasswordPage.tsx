// REQ-FE-02: reset-password form. The token usually arrives as ?token=...
// from the email; we let the user override it manually if needed.
import { useEffect } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { Link, useNavigate, useSearch } from "@tanstack/react-router";
import { toast } from "sonner";
import { useResetPassword } from "@/api";
import { AuthLayout } from "@/components/auth/AuthLayout";
import { Field } from "@/components/ui/Field";
import { SubmitButton } from "@/components/ui/SubmitButton";
import { formatApiError } from "@/lib/errors/api";
import { routes } from "@/lib/routes";
import {
  PASSWORD_MIN_LENGTH,
  resetPasswordFormSchema,
  type ResetPasswordFormValues,
} from "@/lib/validation/auth";

export function ResetPasswordPage(): JSX.Element {
  const search = useSearch({ from: routes.resetPassword });
  const tokenFromUrl = search.token ?? "";
  const navigate = useNavigate();
  const resetPassword = useResetPassword();
  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
    setValue,
  } = useForm<ResetPasswordFormValues>({
    resolver: zodResolver(resetPasswordFormSchema),
    defaultValues: {
      token: tokenFromUrl,
      new_password: "",
      confirm_password: "",
    },
  });

  useEffect(() => {
    if (tokenFromUrl) {
      setValue("token", tokenFromUrl, { shouldValidate: false });
    }
  }, [tokenFromUrl, setValue]);

  const onSubmit = handleSubmit(async (values) => {
    try {
      await resetPassword.mutateAsync({
        token: values.token,
        new_password: values.new_password,
      });
      toast.success("Password updated — please sign in.");
      void navigate({ to: routes.login, replace: true });
    } catch (error) {
      toast.error(
        formatApiError(error, "We couldn't reset your password."),
      );
    }
  });

  return (
    <AuthLayout
      title="Choose a new password"
      subtitle="Enter your reset token and a new password."
      footer={
        <>
          <Link to={routes.login}>Back to sign in</Link>
          <Link to={routes.forgotPassword}>Need a new link?</Link>
        </>
      }
    >
      <form className="auth-form" onSubmit={onSubmit} noValidate>
        <Field
          label="Reset token"
          autoComplete="one-time-code"
          placeholder="Paste the token from the email"
          {...(errors.token?.message ? { error: errors.token.message } : {})}
          {...register("token")}
        />
        <Field
          label="New password"
          type="password"
          autoComplete="new-password"
          helperText={`At least ${String(PASSWORD_MIN_LENGTH)} characters.`}
          {...(errors.new_password?.message
            ? { error: errors.new_password.message }
            : {})}
          {...register("new_password")}
        />
        <Field
          label="Confirm new password"
          type="password"
          autoComplete="new-password"
          {...(errors.confirm_password?.message
            ? { error: errors.confirm_password.message }
            : {})}
          {...register("confirm_password")}
        />
        <SubmitButton isLoading={isSubmitting} loadingLabel="Updating…">
          Update password
        </SubmitButton>
      </form>
    </AuthLayout>
  );
}
