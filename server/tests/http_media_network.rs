pub mod common;

use axum::http::StatusCode;
use common::{Harness, request};
use tower::ServiceExt;

#[tokio::test]
async fn media_network_capability_is_authenticated_and_secret_free() {
    // Given
    let harness = Harness::new();
    let cookie = harness.login().await;

    // When
    let unauthorized = harness
        .app
        .clone()
        .oneshot(request(
            "GET",
            "/api/v1/media/network-capabilities",
            None,
            None,
        ))
        .await
        .expect("unauthorized response");
    let authorized = harness
        .app
        .clone()
        .oneshot(request(
            "GET",
            "/api/v1/media/network-capabilities",
            Some(&cookie),
            None,
        ))
        .await
        .expect("authorized response");

    // Then
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(authorized.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(authorized.into_body(), 1_048_576)
        .await
        .expect("capability body");
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("capability JSON");
    assert_eq!(body["authenticatedSameOriginSignalling"], true);
    assert_eq!(body["controlHttpsProvesMedia"], false);
    assert_eq!(body["remoteLaptopVerified"], false);
    assert!(body.get("turnServers").is_none());
    assert!(body.get("stunServer").is_none());
}
