import {
  useMutation,
  useQuery,
  useQueryClient,
  type UseQueryOptions,
} from "@tanstack/react-query";
import { apiClient, toApiError, type ApiError } from "../client";
import type { components } from "../generated/schema";

type ListApiKeysResponse = components["schemas"]["ListApiKeysResponse"];
type CreateApiKeyRequest = components["schemas"]["CreateApiKeyRequest"];
type CreateApiKeyResponse = components["schemas"]["CreateApiKeyResponse"];

export const apiKeysQueryKey = ["api-keys"] as const;

export function useApiKeys(
  options?: Omit<
    UseQueryOptions<ListApiKeysResponse, ApiError>,
    "queryKey" | "queryFn"
  >,
) {
  return useQuery<ListApiKeysResponse, ApiError>({
    queryKey: apiKeysQueryKey,
    queryFn: async () => {
      const { data, error, response } = await apiClient.GET("/v1/api-keys");
      if (error || !data) {
        throw toApiError(response.status, error);
      }
      return data;
    },
    ...options,
  });
}

export function useCreateApiKey() {
  const qc = useQueryClient();
  return useMutation<CreateApiKeyResponse, ApiError, CreateApiKeyRequest>({
    mutationFn: async (body) => {
      const { data, error, response } = await apiClient.POST(
        "/v1/api-keys",
        { body },
      );
      if (error || !data) {
        throw toApiError(response.status, error);
      }
      return data;
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: apiKeysQueryKey });
    },
  });
}

export function useRevokeApiKey() {
  const qc = useQueryClient();
  return useMutation<void, ApiError, string>({
    mutationFn: async (id) => {
      const { error, response } = await apiClient.DELETE(
        "/v1/api-keys/{id}",
        { params: { path: { id } } },
      );
      if (error) {
        throw toApiError(response.status, error);
      }
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: apiKeysQueryKey });
    },
  });
}
