pub mod common;

use axum::http::StatusCode;
use common::{Harness, request};
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn srt_publish_credential_requires_authenticated_provisioning() {
    // Given
    let harness = Harness::new();
    let cookie = harness.login().await;
    let initial = harness.snapshot(&cookie).await;
    let created = harness
        .command(
            &cookie,
            json!({
                "id": "secure-srt-source", "kind": "source.create",
                "targetId": initial["project"]["id"],
                "expectedEngineRunId": initial["engineRunId"],
                "payload": {"name": "Secure SRT", "adapter": {"kind": "srt", "port": 19000, "bufferMs": 120}}
            }),
        )
        .await;
    assert_eq!(created["status"], "applied", "{created}");
    let snapshot = harness.snapshot(&cookie).await;
    let source_id = snapshot["project"]["sources"][0]["id"]
        .as_str()
        .expect("source id");
    let uri = format!("/api/v1/sources/{source_id}/publishing-endpoint");

    // When
    let unauthorized = harness
        .app
        .clone()
        .oneshot(request("GET", &uri, None, None))
        .await
        .expect("unauthorized response");
    let authorized = harness
        .app
        .clone()
        .oneshot(request("GET", &uri, Some(&cookie), None))
        .await
        .expect("authorized response");

    // Then
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(authorized.status(), StatusCode::OK);
    assert_eq!(authorized.headers()["cache-control"], "no-store, private");
    let bytes = axum::body::to_bytes(authorized.into_body(), 16_384)
        .await
        .expect("credential body");
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("credential JSON");
    let passphrase = body["passphrase"].as_str().expect("passphrase");
    assert_eq!(body["bindAddress"], "0.0.0.0");
    assert!(passphrase.len() >= 32);
    assert!(!passphrase.contains(source_id));
    assert!(!snapshot.to_string().contains(passphrase));
}
