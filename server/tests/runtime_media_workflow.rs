pub mod common;

use common::Harness;
use serde_json::json;

#[tokio::test]
async fn coordinator_source_scene_take_and_recording_drive_real_program_media() {
    // Given
    let harness = Harness::new();
    let cookie = harness.login().await;
    let initial = harness.snapshot(&cookie).await;
    let engine_run_id = initial["engineRunId"].clone();
    let source_receipt = harness
        .command(
            &cookie,
            json!({
                "id": "runtime-source", "kind": "source.create",
                "targetId": initial["project"]["id"], "expectedEngineRunId": engine_run_id,
                "payload": {"name": "Runtime camera", "adapter": {"kind": "synthetic", "pattern": "ball", "frequencyHz": 660}}
            }),
        )
        .await;
    assert_eq!(source_receipt["status"], "applied", "{source_receipt}");
    let sourced = harness.snapshot(&cookie).await;
    let source_id = sourced["project"]["sources"][0]["id"].clone();
    let scene_id = sourced["preview"]["sceneId"].clone();
    let placement = harness
        .command(
            &cookie,
            json!({
                "id": "runtime-placement", "kind": "scene.item.add", "targetId": scene_id,
                "expectedRevision": sourced["preview"]["revision"], "expectedEngineRunId": engine_run_id,
                "payload": {"sourceId": source_id}
            }),
        )
        .await;
    assert_eq!(placement["status"], "applied", "{placement}");
    let ready = harness.snapshot(&cookie).await;
    let scene = &ready["project"]["scenes"][0];

    // When
    let taken = harness
        .command(
            &cookie,
            json!({
                "id": "runtime-take", "kind": "program.take", "targetId": scene["id"],
                "expectedRevision": ready["program"]["programRevision"], "expectedEngineRunId": engine_run_id,
                "payload": {"sceneId": scene["id"], "revision": scene["revision"]}
            }),
        )
        .await;
    let output = harness
        .command(
            &cookie,
            json!({
                "id": "runtime-output", "kind": "output.create", "targetId": ready["project"]["id"],
                "expectedEngineRunId": engine_run_id,
                "payload": {"name": "Runtime Program", "adapter": {"kind": "programRecording", "segmentSeconds": 1}}
            }),
        )
        .await;
    let configured = harness.snapshot(&cookie).await;
    let output_id = configured["project"]["outputs"][0]["id"].clone();
    let started = harness
        .command(
            &cookie,
            json!({
                "id": "runtime-output-start", "kind": "output.start", "targetId": output_id,
                "expectedEngineRunId": engine_run_id, "payload": {}
            }),
        )
        .await;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    let stopped = harness
        .command(
            &cookie,
            json!({
                "id": "runtime-output-stop", "kind": "output.stop", "targetId": output_id,
                "expectedEngineRunId": engine_run_id, "payload": {}
            }),
        )
        .await;

    // Then
    assert_eq!(taken["status"], "applied", "{taken}");
    assert_eq!(output["status"], "applied", "{output}");
    assert_eq!(started["status"], "applied", "{started}");
    assert_eq!(stopped["status"], "applied", "{stopped}");
    let final_snapshot = harness.snapshot(&cookie).await;
    assert_eq!(final_snapshot["program"]["sceneId"], scene["id"]);
    assert_eq!(
        final_snapshot["project"]["sources"][0]["tracks"][0]["connection"],
        "connected"
    );
    assert_eq!(final_snapshot["program"]["health"], "healthy");
    assert!(
        final_snapshot["engineClock"]["mediaTime"].is_string(),
        "{final_snapshot}"
    );
    assert!(final_snapshot["engineClock"]["wallTime"].is_string());
    assert!(final_snapshot["engineClock"]["uncertainty"].is_string());
    let recordings = final_snapshot["recordings"].as_array().expect("recordings");
    assert!(recordings.len() >= 2, "{recordings:?}");
    assert!(
        recordings
            .iter()
            .all(|entry| entry["validation"] == "decoded")
    );
}
