// REQ-FE-02: signup form. On success, route to /verify-email so the user can
// paste the token from the verification email.
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { Link, useNavigate } from "@tanstack/react-router";
import { toast } from "sonner";
import { useSignup } from "@/api";
import { AuthLayout } from "@/components/auth/AuthLayout";
import { Field } from "@/components/ui/Field";
import { SubmitButton } from "@/components/ui/SubmitButton";
import { formatApiError } from "@/lib/errors/api";
import { routes } from "@/lib/routes";
import {
  PASSWORD_MIN_LENGTH,
  signupFormSchema,
  type SignupFormValues,
} from "@/lib/validation/auth";

export function SignupPage(): JSX.Element {
  const navigate = useNavigate();
  const signup = useSignup();
  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<SignupFormValues>({
    resolver: zodResolver(signupFormSchema),
    defaultValues: { email: "", password: "", tenant_name: "" },
  });

  const onSubmit = handleSubmit(async (values) => {
    try {
      await signup.mutateAsync(values);
      toast.success("Account created — check your email for the verification link.");
      void navigate({ to: routes.verifyEmail, replace: true });
    } catch (error) {
      toast.error(formatApiError(error, "Could not create your account."));
    }
  });

  return (
    <AuthLayout
      title="Create your workspace"
      subtitle="Sign up with your work email to start using Rustacean."
      footer={
        <>
          <span>Already have an account?</span>
          <Link to={routes.login}>Sign in</Link>
        </>
      }
    >
      <form className="auth-form" onSubmit={onSubmit} noValidate>
        <Field
          label="Workspace name"
          autoComplete="organization"
          placeholder="Acme Inc."
          {...(errors.tenant_name?.message
            ? { error: errors.tenant_name.message }
            : {})}
          {...register("tenant_name")}
        />
        <Field
          label="Work email"
          type="email"
          autoComplete="email"
          placeholder="you@company.com"
          {...(errors.email?.message ? { error: errors.email.message } : {})}
          {...register("email")}
        />
        <Field
          label="Password"
          type="password"
          autoComplete="new-password"
          helperText={`At least ${String(PASSWORD_MIN_LENGTH)} characters.`}
          {...(errors.password?.message ? { error: errors.password.message } : {})}
          {...register("password")}
        />
        <SubmitButton isLoading={isSubmitting} loadingLabel="Creating account…">
          Create account
        </SubmitButton>
      </form>
    </AuthLayout>
  );
}
