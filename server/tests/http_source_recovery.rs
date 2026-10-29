#![cfg(test)]

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::{Harness, request};
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn operator_and_source_controller_routes_are_scoped_and_nonsecret() {
    // Given
    let harness = Harness::new();
    let cookie = harness.login().await;
    let initial = harness.snapshot(&cookie).await;
    create_source(
        &harness,
        &cookie,
        &initial,
        "source-create-a",
        "Recovery A",
        free_port(),
    )
    .await;
    create_source(
        &harness,
        &cookie,
        &initial,
        "source-create-b",
        "Recovery B",
        free_port(),
    )
    .await;
    let snapshot = harness.snapshot(&cookie).await;
    let source_a = snapshot["project"]["sources"][0]["id"]
        .as_str()
        .expect("source A");
    let source_b = snapshot["project"]["sources"][1]["id"]
        .as_str()
        .expect("source B");
    let default = json_request(
        &harness,
        request(
            "GET",
            &format!("/api/v1/sources/{source_a}/recovery"),
            Some(&cookie),
            None,
        ),
    )
    .await;
    assert_eq!(default.0, StatusCode::OK);
    assert_eq!(default.1["policy"]["enabled"], false);
    assert_eq!(default.1["receiver"]["videoFrames"], "0");
    assert_eq!(default.1["receiver"]["audioBuffers"], "0");
    assert_eq!(default.1["receiver"]["continuityGeneration"], "1");
    let policy = json_request(
        &harness,
        request(
            "PUT",
            &format!("/api/v1/sources/{source_a}/recovery"),
            Some(&cookie),
            Some(json!({"expectedPolicyVersion":"1","enabled":true,"maintenance":false})),
        ),
    )
    .await;
    assert_eq!(policy.0, StatusCode::OK);

    // When
    let enrolled_response = harness
        .app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/v1/sources/{source_a}/recovery/enroll"),
            Some(&cookie),
            Some(json!({"rotate":false})),
        ))
        .await
        .expect("enroll response");
    assert_eq!(enrolled_response.status(), StatusCode::OK);
    assert_eq!(
        enrolled_response.headers()["cache-control"],
        "no-store, private"
    );
    let enrolled = response_json(enrolled_response).await;
    let token = enrolled["controllerToken"]
        .as_str()
        .expect("controller token");
    let controller = enrolled["controllerId"].as_str().expect("controller id");
    let wrong = poll(&harness, source_a, "wrong-token", controller).await;
    let cross_source = poll(&harness, source_b, token, controller).await;
    let accepted = poll(&harness, source_a, token, controller).await;
    let conflict = harness
        .app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/v1/sources/{source_a}/recovery/enroll"),
            Some(&cookie),
            Some(json!({"rotate":false})),
        ))
        .await
        .expect("duplicate enroll");
    let stale_policy = harness
        .app
        .clone()
        .oneshot(request(
            "PUT",
            &format!("/api/v1/sources/{source_a}/recovery"),
            Some(&cookie),
            Some(json!({"expectedPolicyVersion":"0","enabled":true,"maintenance":false})),
        ))
        .await
        .expect("stale policy");
    let retry_conflict = harness
        .app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/v1/sources/{source_a}/recovery/retry"),
            Some(&cookie),
            Some(json!({"requestId":uuid::Uuid::new_v4().to_string()})),
        ))
        .await
        .expect("retry response");
    let revoked = harness
        .app
        .clone()
        .oneshot(request(
            "DELETE",
            &format!("/api/v1/sources/{source_a}/recovery/controller"),
            Some(&cookie),
            None,
        ))
        .await
        .expect("revoke response");
    let revoked_token = poll(&harness, source_a, token, controller).await;

    // Then
    assert_eq!(wrong.0, StatusCode::UNAUTHORIZED);
    assert_eq!(cross_source.0, StatusCode::UNAUTHORIZED);
    assert_eq!(accepted.0, StatusCode::OK);
    assert_eq!(accepted.1["processEpoch"], "1");
    assert!(accepted.1["action"].is_null());
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    assert_eq!(stale_policy.status(), StatusCode::CONFLICT);
    assert_eq!(retry_conflict.status(), StatusCode::CONFLICT);
    assert_eq!(revoked.status(), StatusCode::OK);
    assert_eq!(revoked_token.0, StatusCode::UNAUTHORIZED);
    let status = json_request(
        &harness,
        request(
            "GET",
            &format!("/api/v1/sources/{source_a}/recovery"),
            Some(&cookie),
            None,
        ),
    )
    .await;
    assert!(!status.1.to_string().contains(token));
}

async fn create_source(
    harness: &Harness,
    cookie: &str,
    initial: &Value,
    id: &str,
    name: &str,
    port: u16,
) {
    let response = harness.command(cookie, json!({"id":id,"kind":"source.create","targetId":initial["project"]["id"],"expectedEngineRunId":initial["engineRunId"],"payload":{"name":name,"adapter":{"kind":"srt","bindAddress":"127.0.0.1","port":port,"bufferMs":120}}})).await;
    assert_eq!(response["status"], "applied", "{response}");
}

async fn poll(
    harness: &Harness,
    source: &str,
    token: &str,
    controller: &str,
) -> (StatusCode, Value) {
    let request = Request::builder().method("POST").uri(format!("/api/v1/source-recovery/{source}/poll")).header("content-type", "application/json").header("authorization", format!("Bearer {token}")).body(Body::from(json!({"schemaVersion":1,"controllerId":controller,"processEpoch":"1","pollSeq":"1","intentEpoch":"1","localDesired":"stopped","publisherState":"stopped","lastAction":null}).to_string())).expect("poll request");
    json_request(harness, request).await
}

async fn json_request(harness: &Harness, request: Request<Body>) -> (StatusCode, Value) {
    let response = harness
        .app
        .clone()
        .oneshot(request)
        .await
        .expect("response");
    let status = response.status();
    (status, response_json(response).await)
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), 1_048_576)
        .await
        .expect("body");
    serde_json::from_slice(&bytes).expect("JSON")
}

fn free_port() -> u16 {
    std::net::UdpSocket::bind(("127.0.0.1", 0))
        .expect("UDP port")
        .local_addr()
        .expect("address")
        .port()
}
