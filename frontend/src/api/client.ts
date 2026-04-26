import createClient, { type Client } from "openapi-fetch";
import type { paths } from "./generated/schema";

export type ApiClient = Client<paths>;

function resolveBaseUrl(): string {
  const fromEnv = import.meta.env.VITE_API_BASE_URL;
  if (fromEnv && fromEnv.length > 0) {
    return fromEnv.replace(/\/$/, "");
  }
  return "";
}

export const apiClient: ApiClient = createClient<paths>({
  baseUrl: resolveBaseUrl(),
  credentials: "include",
  headers: {
    "Content-Type": "application/json",
  },
});

export type ApiError = {
  status: number;
  body: unknown;
};

export function toApiError(
  status: number,
  body: unknown,
): ApiError {
  return { status, body };
}
