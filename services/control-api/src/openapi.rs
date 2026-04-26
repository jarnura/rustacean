// The utoipa OpenApi derive macro generates code that triggers
// clippy::needless_for_each internally. Suppress at file scope since this is
// a macro code-generation artefact we cannot control.
#![allow(clippy::needless_for_each)]

use utoipa::OpenApi;

use crate::routes::{auth, health, me, tenants};

#[derive(OpenApi)]
#[openapi(
    paths(
        health::health_check,
        health::ready_check,
        auth::signup,
        auth::forgot_password,
        auth::reset_password,
        me::switch_tenant,
        tenants::invite_member,
        tenants::update_member_role,
        tenants::remove_member,
        tenants::transfer_ownership,
    ),
    components(
        schemas(
            health::ProbeResponse,
            auth::SignupRequest,
            auth::SignupResponse,
            auth::ForgotPasswordRequest,
            auth::ResetPasswordRequest,
            me::SwitchTenantRequest,
            me::SwitchTenantResponse,
            me::CurrentTenant,
            tenants::InviteMemberRequest,
            tenants::InviteMemberResponse,
            tenants::UpdateRoleRequest,
            tenants::UpdateRoleResponse,
            tenants::TransferOwnershipRequest,
        )
    ),
    info(
        title = "rust-brain control API",
        version = "0.1.0",
        description = "Control-plane API for rust-brain: auth, tenant management, and API key endpoints.",
        contact(
            name = "rust-brain",
            url = "https://github.com/jarnura/rustacean",
        ),
    ),
    tags(
        (name = "health", description = "Liveness and readiness probes"),
        (name = "auth", description = "Authentication and session management"),
        (name = "me", description = "Current-user and session endpoints"),
        (name = "tenants", description = "Tenant membership and role management"),
    ),
)]
pub struct ApiDoc;
