pub mod api_keys;
pub mod auth;
pub mod auth_logout;
pub mod auth_verify;
pub mod health;
pub mod me;
pub mod tenants;

use axum::{Router, routing::{delete, get, post, put}};

use crate::routes::{
    api_keys::{create_api_key, list_api_keys, revoke_api_key},
    auth::{forgot_password, login, reset_password, signup},
    auth_logout::logout,
    auth_verify::verify_email,
    health::{health_check, openapi_json, ready_check},
    me::{get_me, switch_tenant},
    tenants::{invite_member, list_members, remove_member, transfer_ownership, update_member_role},
};
use crate::state::AppState;

/// Assembles the full application router.
pub fn build(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/ready", get(ready_check))
        .route("/openapi.json", get(openapi_json))
        .route("/v1/auth/signup", post(signup))
        .route("/v1/auth/login", post(login))
        .route("/v1/auth/logout", post(logout))
        .route("/v1/auth/verify-email", post(verify_email))
        .route("/v1/auth/forgot-password", post(forgot_password))
        .route("/v1/auth/reset-password", post(reset_password))
        .route("/v1/me", get(get_me))
        .route("/v1/me/switch-tenant", post(switch_tenant))
        .route("/v1/api-keys", post(create_api_key))
        .route("/v1/api-keys", get(list_api_keys))
        .route("/v1/api-keys/{id}", delete(revoke_api_key))
        .route("/v1/tenants/{id}/members", get(list_members).post(invite_member))
        .route("/v1/tenants/{id}/members/{uid}/role", put(update_member_role))
        .route("/v1/tenants/{id}/members/{uid}", delete(remove_member))
        .route("/v1/tenants/{id}/transfer-ownership", post(transfer_ownership))
        .with_state(state)
}
