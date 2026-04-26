use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt as _;
use rb_auth::PasswordHasher;
use rb_email::from_transport;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt as _;

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
