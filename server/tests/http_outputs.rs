pub mod common;

use axum::http::StatusCode;
use common::{Harness, request};
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn program_recording_commands_finalize_truthful_catalog() {
    // Given
    let harness = Harness::new();
    let cookie = harness.login().await;
    let initial = harness.snapshot(&cookie).await;
    let created = harness
        .command(
            &cookie,
            json!({
                "id": "recording-create", "kind": "output.create",
                "targetId": initial["project"]["id"],
                "expectedEngineRunId": initial["engineRunId"],
                "payload": {"name": "Program archive", "adapter": {"kind": "programRecording", "segmentSeconds": 1}}
            }),
        )
        .await;
    assert_eq!(created["status"], "applied", "{created}");
    let configured = harness.snapshot(&cookie).await;
    let output_id = configured["project"]["outputs"][0]["id"].clone();

    // When
    let started = harness
        .command(
            &cookie,
            json!({
                "id": "recording-start", "kind": "output.start", "targetId": output_id,
                "expectedEngineRunId": initial["engineRunId"], "payload": {}
            }),
        )
        .await;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    let stopped = harness
        .command(
            &cookie,
            json!({
                "id": "recording-stop", "kind": "output.stop", "targetId": output_id,
                "expectedEngineRunId": initial["engineRunId"], "payload": {}
            }),
        )
        .await;

    // Then
    assert_eq!(started["status"], "applied", "{started}");
    assert_eq!(stopped["status"], "applied", "{stopped}");
    let snapshot = harness.snapshot(&cookie).await;
    assert_eq!(snapshot["outputStatuses"][0]["desired"], "stopped");
    assert_eq!(snapshot["outputStatuses"][0]["actual"], "stopped");
    let recordings = snapshot["recordings"].as_array().expect("recordings");
    assert!(recordings.len() >= 2, "{recordings:?}");
    assert!(
        recordings
            .iter()
            .all(|entry| entry["validation"] == "decoded")
    );
    assert!(recordings.iter().all(|entry| {
        entry["sha256"]
            .as_str()
            .is_some_and(|value| value.len() == 64)
    }));
}

#[tokio::test]
async fn rtmps_destination_provisioning_is_authorized_and_secret_free() {
    // Given
    let harness = Harness::new();
    let cookie = harness.login().await;
    let body =
        json!({"destinationId": "local-primary", "url": "rtmps://127.0.0.1:1936/live/private-key"});

    // When
    let unauthorized = harness
        .app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/output-destinations",
            None,
            Some(body.clone()),
        ))
        .await
        .expect("unauthorized response");
    let authorized = harness
        .app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/output-destinations",
            Some(&cookie),
            Some(body),
        ))
        .await
        .expect("authorized response");

    // Then
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(authorized.status(), StatusCode::NO_CONTENT);
    let snapshot = harness.snapshot(&cookie).await;
    assert!(!snapshot.to_string().contains("private-key"));
    assert!(!snapshot.to_string().contains("rtmps://"));
}

#[tokio::test]
async fn clean_source_recording_uses_native_source_taps() {
    // Given
    let harness = Harness::new();
    let cookie = harness.login().await;
    let initial = harness.snapshot(&cookie).await;
    let source = harness.command(&cookie, json!({
        "id": "clean-source", "kind": "source.create", "targetId": initial["project"]["id"],
        "expectedEngineRunId": initial["engineRunId"],
        "payload": {"name": "Clean fixture", "adapter": {"kind": "synthetic", "pattern": "ball", "frequencyHz": 550}}
    })).await;
    assert_eq!(source["status"], "applied", "{source}");
    let with_source = harness.snapshot(&cookie).await;
    let source_id = with_source["project"]["sources"][0]["id"].clone();
    let created = harness.command(&cookie, json!({
        "id": "clean-output", "kind": "output.create", "targetId": initial["project"]["id"],
        "expectedEngineRunId": initial["engineRunId"],
        "payload": {"name": "Clean archive", "adapter": {"kind": "sourceRecording", "sourceId": source_id, "segmentSeconds": 1}}
    })).await;
    assert_eq!(created["status"], "applied", "{created}");
    let configured = harness.snapshot(&cookie).await;
    let output_id = configured["project"]["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|output| output["automatic"] == false)
        .unwrap()["id"]
        .clone();

    // When
    let started = harness
        .command(
            &cookie,
            json!({
                "id": "clean-start", "kind": "output.start", "targetId": output_id,
                "expectedEngineRunId": initial["engineRunId"], "payload": {}
            }),
        )
        .await;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    let stopped = harness
        .command(
            &cookie,
            json!({
                "id": "clean-stop", "kind": "output.stop", "targetId": output_id,
                "expectedEngineRunId": initial["engineRunId"], "payload": {}
            }),
        )
        .await;

    // Then
    assert_eq!(started["status"], "applied", "{started}");
    assert_eq!(stopped["status"], "applied", "{stopped}");
    let snapshot = harness.snapshot(&cookie).await;
    let recordings: Vec<_> = snapshot["recordings"]
        .as_array()
        .expect("recordings")
        .iter()
        .filter(|entry| entry["outputId"] == output_id)
        .collect();
    assert!(recordings.len() >= 2, "{recordings:?}");
    assert!(
        recordings
            .iter()
            .all(|entry| entry["tapPoint"] == "clean_source")
    );
    assert!(
        recordings
            .iter()
            .all(|entry| entry["validation"] == "decoded")
    );
}
