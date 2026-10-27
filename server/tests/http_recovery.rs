pub mod common;

use common::Harness;
use live_production::{
    coordinator::Coordinator,
    media_bridge::MediaBridge,
    routes::{self, AppState},
};
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn output_catalog_restart_and_command_deduplication_are_safe() {
    // Given
    let directory = Arc::new(tempfile::tempdir().expect("state directory"));
    let database = directory.path().join("project.sqlite");
    let first = Coordinator::spawn(database.clone(), MediaBridge::spawn().expect("media"))
        .expect("first coordinator");
    let first_harness = Harness {
        app: routes::router(
            AppState::new("operator-password", false, first),
            directory.path().join("dist"),
        ),
        _directory: directory.clone(),
    };
    let cookie = first_harness.login().await;
    let initial = first_harness.snapshot(&cookie).await;

    // When
    let create = json!({
        "id": "deduplicated-scene", "kind": "scene.create", "targetId": "project",
        "expectedEngineRunId": initial["engineRunId"], "payload": {"name": "Interview"}
    });
    let first_receipt = first_harness.command(&cookie, create.clone()).await;
    let duplicate_receipt = first_harness.command(&cookie, create).await;
    let output = first_harness.command(&cookie, json!({
        "id": "output-1", "kind": "output.create", "targetId": initial["project"]["id"],
        "expectedEngineRunId": initial["engineRunId"],
        "payload": {"name": "Program archive", "adapter": {"kind": "programRecording", "segmentSeconds": 60}}
    })).await;
    let configured = first_harness.snapshot(&cookie).await;
    let started = first_harness.command(&cookie, json!({
        "id": "output-start-1", "kind": "output.start", "targetId": configured["project"]["outputs"][0]["id"],
        "expectedEngineRunId": configured["engineRunId"], "payload": {}
    })).await;
    let stopped = first_harness.command(&cookie, json!({
        "id": "output-stop-1", "kind": "output.stop", "targetId": configured["project"]["outputs"][0]["id"],
        "expectedEngineRunId": configured["engineRunId"], "payload": {}
    })).await;
    let first_run = configured["engineRunId"].clone();
    drop(first_harness);
    let restarted = Coordinator::spawn(database, MediaBridge::spawn().expect("restarted media"))
        .expect("restarted coordinator");
    let restarted_harness = Harness {
        app: routes::router(
            AppState::new("operator-password", false, restarted),
            directory.path().join("dist"),
        ),
        _directory: directory,
    };
    let restarted_cookie = restarted_harness.login().await;
    let after_restart = restarted_harness.snapshot(&restarted_cookie).await;

    // Then
    assert_eq!(first_receipt, duplicate_receipt);
    assert_eq!(
        configured["project"]["scenes"]
            .as_array()
            .expect("scenes")
            .len(),
        2
    );
    assert_eq!(output["status"], "applied");
    assert_eq!(started["status"], "applied");
    assert_eq!(stopped["status"], "applied");
    assert_eq!(configured["outputStatuses"][0]["actual"], "stopped");
    assert!(
        configured["recordings"]
            .as_array()
            .expect("recordings")
            .is_empty()
    );
    assert_ne!(after_restart["engineRunId"], first_run);
    assert_eq!(after_restart["outputStatuses"][0]["desired"], "stopped");
}
