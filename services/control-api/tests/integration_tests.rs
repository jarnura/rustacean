use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt as _;
use tower::ServiceExt as _;

fn app() -> axum::Router {
    control_api::routes::build()
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
