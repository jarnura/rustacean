import { useMutation, useQueryClient } from "@tanstack/react-query";
import { apiClient, toApiError, type ApiError } from "../client";
import type { components } from "../generated/schema";
import { meQueryKey } from "./useMe";

type InviteMemberRequest = components["schemas"]["InviteMemberRequest"];
type InviteMemberResponse = components["schemas"]["InviteMemberResponse"];
type UpdateRoleRequest = components["schemas"]["UpdateRoleRequest"];
type UpdateRoleResponse = components["schemas"]["UpdateRoleResponse"];
type TransferOwnershipRequest =
  components["schemas"]["TransferOwnershipRequest"];

export function useInviteMember(tenantId: string) {
  return useMutation<InviteMemberResponse, ApiError, InviteMemberRequest>({
    mutationFn: async (body) => {
      const { data, error, response } = await apiClient.POST(
        "/v1/tenants/{id}/members",
        { params: { path: { id: tenantId } }, body },
      );
      if (error || !data) {
        throw toApiError(response.status, error);
      }
      return data;
    },
  });
}

export function useUpdateMemberRole(tenantId: string) {
  return useMutation<
    UpdateRoleResponse,
    ApiError,
    { uid: string; body: UpdateRoleRequest }
  >({
    mutationFn: async ({ uid, body }) => {
      const { data, error, response } = await apiClient.PUT(
        "/v1/tenants/{id}/members/{uid}/role",
        { params: { path: { id: tenantId, uid } }, body },
      );
      if (error || !data) {
        throw toApiError(response.status, error);
      }
      return data;
    },
  });
}

export function useRemoveMember(tenantId: string) {
  return useMutation<void, ApiError, string>({
    mutationFn: async (uid) => {
      const { error, response } = await apiClient.DELETE(
        "/v1/tenants/{id}/members/{uid}",
        { params: { path: { id: tenantId, uid } } },
      );
      if (error) {
        throw toApiError(response.status, error);
      }
    },
  });
}

export function useTransferOwnership(tenantId: string) {
  const qc = useQueryClient();
  return useMutation<void, ApiError, TransferOwnershipRequest>({
    mutationFn: async (body) => {
      const { error, response } = await apiClient.POST(
        "/v1/tenants/{id}/transfer-ownership",
        { params: { path: { id: tenantId } }, body },
      );
      if (error) {
        throw toApiError(response.status, error);
      }
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: meQueryKey });
    },
  });
}
