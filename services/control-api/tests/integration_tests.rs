use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt as _;
use rb_auth::{LoginRateLimiter, PasswordHasher};
use rb_email::from_transport;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt as _;
use uuid::Uuid;

use control_api::{AppState, Config, build};

fn test_state() -> AppState {
    let config = Config::for_test();
    let pool = PgPoolOptions::new()
        .connect_lazy(&config.database_url)
        .expect("connect_lazy must succeed");
    let smtp = rb_email::SmtpConfig {
        host: String::new(),
        port: 587,
        username: String::new(),
        password: String::new(),
        from_address: "test@example.com".to_owned(),
    };
    let email_sender =
        from_transport("noop", &smtp).expect("noop transport must succeed");
    let hasher = PasswordHasher::from_config(64, 1, 1).expect("hasher must build");
    AppState {
        pool,
        email_sender: Arc::from(email_sender),
        hasher: Arc::new(hasher),
        login_rate_limiter: Arc::new(LoginRateLimiter::new()),
        config: Arc::new(config),
    }
}

fn app() -> axum::Router {
    build(test_state())
}

async fn collect_body(body: Body) -> Vec<u8> {
    body.collect()
        .await
        .expect("collect body")
        .to_bytes()
        .to_vec()
}

#[tokio::test]
async fn health_returns_200() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn ready_returns_200() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn openapi_json_content_type_is_application_json() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .expect("content-type header must be present")
        .to_str()
        .unwrap();
    assert!(
        content_type.contains("application/json"),
        "expected application/json, got {content_type}"
    );
}

#[tokio::test]
async fn openapi_json_body_is_valid_openapi() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let raw = collect_body(response.into_body()).await;
    let spec: serde_json::Value = serde_json::from_slice(&raw).expect("body must be valid JSON");

    let version = spec["openapi"].as_str().expect("'openapi' field must be a string");
    assert!(
        version.starts_with("3."),
        "expected OpenAPI 3.x, got {version}"
    );
    assert!(spec["info"].is_object(), "'info' must be present and an object");
    assert!(spec["paths"].is_object(), "'paths' must be present and an object");
}

#[tokio::test]
async fn openapi_json_includes_signup_path() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let raw = collect_body(response.into_body()).await;
    let spec: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert!(
        spec["paths"]["/v1/auth/signup"].is_object(),
        "signup path must be present in OpenAPI spec"
    );
}

#[tokio::test]
async fn openapi_json_includes_verify_email_path() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let raw = collect_body(response.into_body()).await;
    let spec: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert!(
        spec["paths"]["/v1/auth/verify-email"].is_object(),
        "verify-email path must be present in OpenAPI spec"
    );
}

#[tokio::test]
async fn verify_email_without_body_returns_400() {
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/verify-email")
                .header("content-type", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Axum returns 400 when the JSON body is absent/empty (cannot parse EOF as JSON).
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn verify_email_missing_token_field_returns_422() {
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/verify-email")
                .header("content-type", "application/json")
                .body(Body::from(r#"{}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn unknown_route_returns_404() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn openapi_json_includes_logout_path() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let raw = collect_body(response.into_body()).await;
    let spec: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert!(
        spec["paths"]["/v1/auth/logout"]["post"].is_object(),
        "logout POST must be present in OpenAPI spec",
    );
}

#[tokio::test]
async fn logout_without_session_cookie_returns_401() {
    // No `Cookie` header → AuthContext::Anonymous resolves without touching the
    // database, so this exercises the unauthorized branch end-to-end without a
    // running Postgres.
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/logout")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn logout_rejects_get_method() {
    let response = app()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/auth/logout")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

// ---------------------------------------------------------------------------
// Real-DB integration tests — skipped when RB_DATABASE_URL is not set
// ---------------------------------------------------------------------------

/// Build a state connected to a real Postgres instance.
///
/// Returns `None` when `RB_DATABASE_URL` is absent so callers can skip
/// gracefully instead of panicking.
async fn real_db_state() -> Option<(AppState, PgPool)> {
    let db_url = std::env::var("RB_DATABASE_URL").ok()?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .ok()?;
    let smtp = rb_email::SmtpConfig {
        host: String::new(),
        port: 587,
        username: String::new(),
        password: String::new(),
        from_address: "test@example.com".to_owned(),
    };
    let email_sender = from_transport("noop", &smtp).ok()?;
    let hasher = PasswordHasher::from_config(64, 1, 1).ok()?;
    let config = Config {
        listen_addr: "127.0.0.1:0".to_owned(),
        database_url: db_url,
        cors_origins: vec![],
        base_url: "http://localhost:8080".to_owned(),
        session_ttl_days: 30,
        argon2_memory_kb: 64,
        argon2_time_cost: 1,
        argon2_parallelism: 1,
        email_transport: "noop".to_owned(),
        service_name: "control-api-test".to_owned(),
    };
    let state = AppState {
        pool: pool.clone(),
        email_sender: Arc::from(email_sender),
        hasher: Arc::new(hasher),
        login_rate_limiter: Arc::new(LoginRateLimiter::new()),
        config: Arc::new(config),
    };
    Some((state, pool))
}

fn json_body(v: serde_json::Value) -> Body {
    Body::from(serde_json::to_vec(&v).expect("serialise JSON"))
}

/// Full login flow: signup → SQL-verify email → login → assert Secure cookie.
///
/// Uses `email_verified_at` SQL patch because the noop transport discards the
/// verification email; the verify-email endpoint itself is covered by RUSAA-30.
#[tokio::test]
async fn integration_login_full_flow() {
    let Some((state, pool)) = real_db_state().await else {
        return;
    };
    let app = build(state);
    let email = format!("integ-login-{}@test.example", Uuid::new_v4().simple());
    let password = "correct-horse-battery-staple";

    // 1. Signup
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/signup")
                .header("content-type", "application/json")
                .body(json_body(serde_json::json!({
                    "email": email,
                    "password": password,
                    "tenant_name": "Integration Login Tenant",
                })))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "signup must return 201");

    // 2. Patch email_verified_at directly (noop transport discards the email)
    sqlx::query("UPDATE control.users SET email_verified_at = NOW() WHERE email = $1")
        .bind(&email)
        .execute(&pool)
        .await
        .expect("email verification patch must succeed");

    // 3. Login
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("content-type", "application/json")
                .body(json_body(serde_json::json!({
                    "email": email,
                    "password": password,
                })))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "login must return 200");

    let cookie = resp
        .headers()
        .get("set-cookie")
        .expect("Set-Cookie header must be present")
        .to_str()
        .unwrap();
    assert!(cookie.contains("rb_session="), "cookie must contain rb_session token");
    assert!(cookie.contains("Secure"), "cookie must carry the Secure flag");
    assert!(cookie.contains("HttpOnly"), "cookie must carry HttpOnly");

    let raw = collect_body(resp.into_body()).await;
    let body: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert!(body["user_id"].is_string(), "user_id must be present");
    assert!(body["tenant_id"].is_string(), "tenant_id must be present");
    assert_eq!(
        body["email_verification_required"], false,
        "email_verification_required must be false after verification"
    );
}

/// Rate-limit path: 5 failed attempts → 6th is blocked with 429.
#[tokio::test]
async fn integration_login_rate_limit() {
    let Some((state, _pool)) = real_db_state().await else {
        return;
    };
    let app = build(state);
    let email = format!("integ-ratelimit-{}@test.example", Uuid::new_v4().simple());
    let password = "correct-horse-battery-staple";

    // Signup so the email resolves in the DB (rate limiter fires on bad password,
    // not on "user not found" — both paths record a failure, but this makes the
    // argon2 verify path exercise the full record_attempt logic).
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/signup")
                .header("content-type", "application/json")
                .body(json_body(serde_json::json!({
                    "email": email,
                    "password": password,
                    "tenant_name": "Rate Limit Test Tenant",
                })))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // 5 wrong-password attempts — each must return 401
    for i in 0..5 {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(json_body(serde_json::json!({
                        "email": email,
                        "password": "wrong-password-xyz",
                    })))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "attempt {i} must be 401"
        );
    }

    // 6th attempt — must be rate-limited (429) regardless of credentials
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("content-type", "application/json")
                .body(json_body(serde_json::json!({
                    "email": email,
                    "password": "wrong-password-xyz",
                })))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS, "6th attempt must be rate-limited");
}

/// Full logout flow: signup → SQL-verify email → login → logout → assert
/// 204 + cookie cleared + session row revoked + `auth_events` row written.
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn integration_logout_full_flow() {
    let Some((state, pool)) = real_db_state().await else {
        return;
    };
    let app = build(state);
    let email = format!("integ-logout-{}@test.example", Uuid::new_v4().simple());
    let password = "correct-horse-battery-staple";

    // 1. Signup
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/signup")
                .header("content-type", "application/json")
                .body(json_body(serde_json::json!({
                    "email": email,
                    "password": password,
                    "tenant_name": "Integration Logout Tenant",
                })))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "signup must return 201");

    // 2. Verify email directly (noop transport discards the verification email)
    sqlx::query("UPDATE control.users SET email_verified_at = NOW() WHERE email = $1")
        .bind(&email)
        .execute(&pool)
        .await
        .expect("email verification patch must succeed");

    // 3. Login → capture session cookie
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("content-type", "application/json")
                .body(json_body(serde_json::json!({
                    "email": email,
                    "password": password,
                })))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "login must return 200");
    let login_set_cookie = resp
        .headers()
        .get("set-cookie")
        .expect("login must set rb_session cookie")
        .to_str()
        .unwrap()
        .to_owned();
    let token = login_set_cookie
        .split(';')
        .next()
        .and_then(|kv| kv.trim().strip_prefix("rb_session="))
        .expect("rb_session token must parse out of Set-Cookie")
        .to_owned();

    // 4. Logout with that cookie
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/logout")
                .header("cookie", format!("rb_session={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT, "logout must return 204");

    let clear = resp
        .headers()
        .get("set-cookie")
        .expect("logout must Set-Cookie to clear rb_session")
        .to_str()
        .unwrap();
    assert!(clear.starts_with("rb_session=;"), "Set-Cookie was: {clear}");
    assert!(clear.contains("Max-Age=0"), "Set-Cookie was: {clear}");
    assert!(clear.contains("Secure"), "Set-Cookie was: {clear}");

    // 5. Session row revoked
    let token_hash = rb_auth::sha256_hex(&token);
    let revoked: bool = sqlx::query_scalar(
        "SELECT revoked_at IS NOT NULL FROM control.sessions WHERE token_hash = $1",
    )
    .bind(&token_hash)
    .fetch_one(&pool)
    .await
    .expect("session row must exist");
    assert!(revoked, "session must be revoked after logout");

    // 6. Logout audit event written exactly once for this user
    let logout_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM control.auth_events e \
         JOIN control.users u ON u.id = e.user_id \
         WHERE u.email = $1 AND e.event = 'logout'",
    )
    .bind(&email)
    .fetch_one(&pool)
    .await
    .expect("auth_events count must succeed");
    assert_eq!(logout_count, 1, "exactly one logout auth_events row expected");

    // 7. Re-logout with the same (now invalid) cookie → 401, no duplicate event.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/logout")
                .header("cookie", format!("rb_session={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "second logout must reject the now-revoked session",
    );

    let logout_count_after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM control.auth_events e \
         JOIN control.users u ON u.id = e.user_id \
         WHERE u.email = $1 AND e.event = 'logout'",
    )
    .bind(&email)
    .fetch_one(&pool)
    .await
    .expect("auth_events count must succeed");
    assert_eq!(
        logout_count_after, 1,
        "logout audit must remain idempotent after replay",
    );
}
