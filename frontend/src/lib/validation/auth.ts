// REQ-FE-02: client-side validation schemas mirroring OpenAPI auth contracts.
// Server-side validation is still authoritative; these provide fast UX feedback
// before the request leaves the browser.
import { z } from "zod";

// PRD-aligned minimum password length. Matches the backend argon2id flow
// where the server rejects passwords shorter than 12 characters.
export const PASSWORD_MIN_LENGTH = 12;

const emailSchema = z
  .email("Enter a valid email address")
  .trim()
  .min(1, "Email is required");

const passwordSchema = z
  .string()
  .min(
    PASSWORD_MIN_LENGTH,
    `Password must be at least ${String(PASSWORD_MIN_LENGTH)} characters`,
  );

const tenantNameSchema = z
  .string()
  .trim()
  .min(1, "Workspace name is required")
  .max(100, "Workspace name must be 100 characters or fewer");

const tokenSchema = z
  .string()
  .trim()
  .min(1, "Token is required");

export const signupFormSchema = z.object({
  email: emailSchema,
  password: passwordSchema,
  tenant_name: tenantNameSchema,
});
export type SignupFormValues = z.infer<typeof signupFormSchema>;

export const loginFormSchema = z.object({
  email: emailSchema,
  password: z.string().min(1, "Password is required"),
});
export type LoginFormValues = z.infer<typeof loginFormSchema>;

export const forgotPasswordFormSchema = z.object({
  email: emailSchema,
});
export type ForgotPasswordFormValues = z.infer<typeof forgotPasswordFormSchema>;

export const resetPasswordFormSchema = z
  .object({
    token: tokenSchema,
    new_password: passwordSchema,
    confirm_password: z.string().min(1, "Please re-enter the new password"),
  })
  .refine((values) => values.new_password === values.confirm_password, {
    message: "Passwords do not match",
    path: ["confirm_password"],
  });
export type ResetPasswordFormValues = z.infer<typeof resetPasswordFormSchema>;

export const verifyEmailFormSchema = z.object({
  token: tokenSchema,
});
export type VerifyEmailFormValues = z.infer<typeof verifyEmailFormSchema>;
