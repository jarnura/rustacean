// REQ-FE-09: validation schemas for the members and API keys management forms.
// Server-side validation is still authoritative; these only catch obvious
// mistakes before the request leaves the browser.
import { z } from "zod";

const emailSchema = z
  .email("Enter a valid email address")
  .trim()
  .min(1, "Email is required");

export const inviteMemberFormSchema = z.object({
  email: emailSchema,
});
export type InviteMemberFormValues = z.infer<typeof inviteMemberFormSchema>;

export const API_KEY_NAME_MAX = 100;

const apiKeyScopes = ["read", "write", "admin"] as const;
export type ApiKeyScope = (typeof apiKeyScopes)[number];

export const createApiKeyFormSchema = z.object({
  name: z
    .string()
    .trim()
    .min(1, "Name is required")
    .max(
      API_KEY_NAME_MAX,
      `Name must be ${String(API_KEY_NAME_MAX)} characters or fewer`,
    ),
  scopes: z
    .array(z.enum(apiKeyScopes))
    .min(1, "Select at least one scope"),
});
export type CreateApiKeyFormValues = z.infer<typeof createApiKeyFormSchema>;
