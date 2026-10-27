pub mod common;

use common::Harness;
use serde_json::json;

#[tokio::test]
async fn authenticated_workflow_applies_drafts_and_native_timing() {
    // Given
    let harness = Harness::new();
    let cookie = harness.login().await;
    let initial = harness.snapshot(&cookie).await;
    let project_id = initial["project"]["id"].as_str().expect("project id");
    let engine_run_id = initial["engineRunId"].as_str().expect("engine run id");

    // When
    let created = harness
        .command(
            &cookie,
            json!({
                "id": "create-scene-1", "kind": "scene.create", "targetId": project_id,
                "expectedEngineRunId": engine_run_id, "payload": {"name": "Camera"}
            }),
        )
        .await;
    let timing = harness
        .command(
            &cookie,
            json!({
                "id": "timing-1", "kind": "timing.apply", "targetId": project_id,
                "expectedRevision": created["actualRevision"], "expectedEngineRunId": engine_run_id,
                "payload": {"holdbackMs": 500, "corrections": []}
            }),
        )
        .await;

    // Then
    assert_eq!(created["status"], "applied");
    assert_eq!(timing["status"], "applied", "{timing}");
    let current = harness.snapshot(&cookie).await;
    assert_eq!(
        current["project"]["scenes"]
            .as_array()
            .expect("scenes")
            .len(),
        2
    );
    assert_eq!(current["project"]["holdbackMs"], 500);
}

#[tokio::test]
async fn take_applies_exact_revision_and_keeps_preview_selected() {
    // Given
    let harness = Harness::new();
    let cookie = harness.login().await;
    let initial = harness.snapshot(&cookie).await;
    let created = harness
        .command(
            &cookie,
            json!({
                "id": "take-scene-create", "kind": "scene.create", "targetId": "project",
                "expectedEngineRunId": initial["engineRunId"], "payload": {"name": "Camera"}
            }),
        )
        .await;
    assert_eq!(created["status"], "applied");
    let snapshot = harness.snapshot(&cookie).await;
    let scene = &snapshot["project"]["scenes"][1];
    let cue = harness.command(&cookie, json!({
        "id": "cue-1", "kind": "scene.cue", "targetId": scene["id"],
        "expectedRevision": scene["revision"], "expectedEngineRunId": snapshot["engineRunId"],
        "payload": {}
    })).await;
    assert_eq!(cue["status"], "applied");

    // When
    let receipt = harness
        .command(
            &cookie,
            json!({
                "id": "take-1", "kind": "program.take", "targetId": scene["id"],
                "expectedRevision": snapshot["program"]["programRevision"],
                "expectedEngineRunId": snapshot["engineRunId"],
                "payload": {"sceneId": scene["id"], "revision": scene["revision"]}
            }),
        )
        .await;

    // Then
    assert_eq!(receipt["status"], "applied", "{receipt}");
    let after = harness.snapshot(&cookie).await;
    assert_eq!(after["program"]["sceneId"], scene["id"]);
    assert_eq!(after["preview"]["sceneId"], scene["id"]);
    assert_eq!(
        after["program"]["previousSceneId"],
        snapshot["program"]["sceneId"]
    );
}

#[tokio::test]
async fn ui_source_scene_and_unavailable_media_commands_are_truthful() {
    // Given
    let harness = Harness::new();
    let cookie = harness.login().await;
    let initial = harness.snapshot(&cookie).await;

    // When
    let source = harness
        .command(
            &cookie,
            json!({
                "id": "source-1", "kind": "source.create", "targetId": initial["project"]["id"],
                "expectedEngineRunId": initial["engineRunId"],
                "payload": {"name": "New SRT input", "adapter": {"kind": "srt"}}
            }),
        )
        .await;
    let after_source = harness.snapshot(&cookie).await;
    let source_id = after_source["project"]["sources"][0]["id"].clone();
    let video_track = after_source["project"]["sources"][0]["tracks"][0]["id"].clone();
    let audio_track = after_source["project"]["sources"][0]["tracks"][1]["id"].clone();
    let placement = harness.command(&cookie, json!({
        "id": "placement-1", "kind": "scene.item.add", "targetId": after_source["preview"]["sceneId"],
        "expectedEngineRunId": initial["engineRunId"], "payload": {"sourceId": source_id}
    })).await;
    let routed = harness.command(&cookie, json!({
        "id": "audio-route-1", "kind": "audio.apply", "targetId": initial["project"]["id"],
        "expectedRevision": source["actualRevision"], "expectedEngineRunId": initial["engineRunId"],
        "payload": {"routes": [{"trackId": audio_track.clone(), "gainDb": 0.0, "muted": false, "routed": true}]}
    })).await;
    let audio = harness
        .command(
            &cookie,
            json!({
                "id": "audio-1", "kind": "audio.gain.set", "targetId": audio_track,
                "expectedEngineRunId": initial["engineRunId"], "payload": {"gainDb": -3.0}
            }),
        )
        .await;
    let timing = harness.command(&cookie, json!({
        "id": "timing-negative", "kind": "timing.apply", "targetId": initial["project"]["id"],
        "expectedRevision": audio["actualRevision"], "expectedEngineRunId": initial["engineRunId"],
        "payload": {"holdbackMs": 100, "corrections": [{"trackId": video_track, "correctionMs": -500}]}
    })).await;

    // Then
    assert_eq!(source["status"], "applied");
    assert_eq!(placement["status"], "applied");
    assert_eq!(routed["status"], "applied", "{routed}");
    assert_eq!(audio["status"], "applied", "{audio}");
    assert_eq!(timing["status"], "rejected");
    let current = harness.snapshot(&cookie).await;
    assert!(current["project"]["audioRoutes"][0]["inputPeakDb"].is_null());
    assert!(current["project"]["audioRoutes"][0]["outputPeakDb"].is_null());
    assert_eq!(
        current["project"]["scenes"][0]["items"]
            .as_array()
            .expect("items")
            .len(),
        1
    );
    assert_eq!(
        current["project"]["sources"][0]["tracks"][0]["connection"],
        "connecting"
    );
}
