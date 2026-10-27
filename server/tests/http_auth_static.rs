pub mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::{Harness, request};
use tower::ServiceExt;

#[tokio::test]
async fn mutation_without_csrf_header_is_rejected() {
    // Given
    let harness = Harness::new();

    // When
    let response = harness
        .app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"password":"operator-password"}"#))
                .expect("request"),
        )
        .await
        .expect("response");

    // Then
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn spa_navigation_serves_index() {
    // Given
    let harness = Harness::new();

    // When
    let response = harness
        .app
        .oneshot(request("GET", "/production/setup", None, None))
        .await
        .expect("SPA response");

    // Then
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .expect("SPA body");
    assert!(bytes.starts_with(b"<main>live production"));
}

#[tokio::test]
async fn authenticated_events_route_opens_sse_stream() {
    // Given
    let harness = Harness::new();
    let cookie = harness.login().await;

    // When
    let response = harness
        .app
        .oneshot(request(
            "GET",
            "/api/v1/events?after=0",
            Some(&cookie),
            None,
        ))
        .await
        .expect("SSE response");

    // Then
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
}
