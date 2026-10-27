#![cfg(test)]

mod common;

use common::Harness;
use live_production::{
    command::Command, coordinator::Coordinator, media_bridge::MediaBridge,
    plugins::EDITOR_PLUGIN_ID,
};
use serde_json::{Value, json};

fn command(value: Value) -> Command {
    serde_json::from_value(value).expect("valid command fixture")
}

#[tokio::test]
async fn editor_export_explicitly_reports_missing_program_reference() {
    // Given
    let directory = tempfile::tempdir().expect("temporary state directory");
    let coordinator = Coordinator::spawn(
        directory.path().join("project.sqlite"),
        MediaBridge::spawn().expect("media engine"),
    )
    .expect("coordinator");
    let initial = coordinator.snapshot().await.expect("initial snapshot");
    let project_id = initial.project.id.0.clone();
    for (id, action, text) in [
        ("start", "session.start", Some("show-1")),
        ("marker", "marker.add", Some("Opening")),
    ] {
        coordinator.execute(command(json!({
            "id": format!("editor-{id}"), "kind": "plugin.command", "targetId": EDITOR_PLUGIN_ID,
            "expectedEngineRunId": initial.engine_run_id, "payload": {"command": action, "text": text}
        }))).await.expect("editor command");
    }

    // When
    let response = coordinator
        .execute(command(json!({
            "id": "editor-export-1", "kind": "plugin.command", "targetId": EDITOR_PLUGIN_ID,
            "expectedEngineRunId": initial.engine_run_id, "payload": {"command": "session.export"}
        })))
        .await
        .expect("export editor session");

    // Then
    assert_eq!(response["status"], "applied", "{response}");
    assert_eq!(response["pluginResult"]["sessionId"], "show-1");
    assert_eq!(response["pluginResult"]["projectId"], project_id);
    assert_eq!(response["pluginResult"]["programReference"], Value::Null);
    assert!(
        response["pluginResult"]["issues"]
            .as_array()
            .expect("issues")
            .iter()
            .any(|issue| issue["kind"] == "missingProgramReference")
    );
    assert_eq!(response["pluginResult"]["markers"][0]["label"], "Opening");
    for history in ["layoutUse", "audioHistory", "timingHistory", "gaps"] {
        assert!(response["pluginResult"][history].is_array());
    }
}

#[tokio::test]
async fn stopped_editor_plugin_leaves_core_commands_available() {
    // Given
    let directory = tempfile::tempdir().expect("temporary state directory");
    let coordinator = Coordinator::spawn(
        directory.path().join("project.sqlite"),
        MediaBridge::spawn().expect("media engine"),
    )
    .expect("coordinator");
    let initial = coordinator.snapshot().await.expect("initial snapshot");
    for (id, action, text) in [
        ("start", "session.start", Some("show")),
        ("stop", "session.stop", None),
    ] {
        coordinator.execute(command(json!({
            "id": format!("editor-{id}"), "kind": "plugin.command", "targetId": EDITOR_PLUGIN_ID,
            "expectedEngineRunId": initial.engine_run_id, "payload": {"command": action, "text": text}
        }))).await.expect("editor lifecycle command");
    }

    // When
    let response = coordinator
        .execute(command(json!({
            "id": "scene-after-editor-stop", "kind": "scene.create", "targetId": initial.project.id,
            "expectedEngineRunId": initial.engine_run_id, "payload": {"name": "Still broadcasting"}
        })))
        .await
        .expect("core command");
    let created_output = coordinator
        .execute(command(json!({
            "id": "output-after-editor-stop", "kind": "output.create", "targetId": initial.project.id,
            "expectedEngineRunId": initial.engine_run_id,
            "payload": {"name": "Independent Program", "adapter": {"kind": "programRecording", "segmentSeconds": 1}}
        })))
        .await
        .expect("create output");
    assert_eq!(created_output["status"], "applied", "{created_output}");
    let configured = coordinator.snapshot().await.expect("configured output");
    let output_id = configured.project.outputs[0].id.clone();
    for (id, action) in [("start", "output.start"), ("stop", "output.stop")] {
        let applied = coordinator
            .execute(command(json!({
                "id": format!("independent-output-{id}"), "kind": action, "targetId": output_id,
                "expectedEngineRunId": initial.engine_run_id, "payload": {}
            })))
            .await
            .expect("output lifecycle");
        assert_eq!(applied["status"], "applied", "{applied}");
        if action == "output.start" {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
    }

    // Then
    assert_eq!(response["status"], "applied", "{response}");
    let snapshot = coordinator.snapshot().await.expect("current snapshot");
    assert!(
        snapshot
            .plugins
            .iter()
            .any(|plugin| plugin.id == EDITOR_PLUGIN_ID && !plugin.enabled)
    );
    assert_eq!(snapshot.project.scenes.len(), 2);
    assert!(!snapshot.recordings.is_empty());
}

#[tokio::test]
async fn editor_export_reconciles_real_program_recording() {
    // Given
    let harness = Harness::new();
    let cookie = harness.login().await;
    let initial = harness.snapshot(&cookie).await;
    let created = harness.command(&cookie, json!({
        "id": "editor-program-output", "kind": "output.create", "targetId": initial["project"]["id"],
        "expectedEngineRunId": initial["engineRunId"],
        "payload": {"name": "Editor Program", "adapter": {"kind": "programRecording", "segmentSeconds": 1}}
    })).await;
    assert_eq!(created["status"], "applied", "{created}");
    let configured = harness.snapshot(&cookie).await;
    let output_id = configured["project"]["outputs"][0]["id"].clone();
    for (id, target, action, text) in [
        (
            "session-start",
            json!(EDITOR_PLUGIN_ID),
            "plugin.command",
            Some(json!({"command": "session.start", "text": "real-recording"})),
        ),
        (
            "output-start",
            output_id.clone(),
            "output.start",
            Some(json!({})),
        ),
    ] {
        let response = harness
            .command(
                &cookie,
                json!({
                    "id": id, "kind": action, "targetId": target,
                    "expectedEngineRunId": initial["engineRunId"], "payload": text
                }),
            )
            .await;
        assert_eq!(response["status"], "applied", "{response}");
    }
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    let stopped = harness
        .command(
            &cookie,
            json!({
                "id": "real-output-stop", "kind": "output.stop", "targetId": output_id,
                "expectedEngineRunId": initial["engineRunId"], "payload": {}
            }),
        )
        .await;
    assert_eq!(stopped["status"], "applied", "{stopped}");

    // When
    let exported = harness.command(&cookie, json!({
        "id": "real-editor-export", "kind": "plugin.command", "targetId": EDITOR_PLUGIN_ID,
        "expectedEngineRunId": initial["engineRunId"], "payload": {"command": "session.export"}
    })).await;

    // Then
    assert_eq!(exported["status"], "applied", "{exported}");
    assert!(exported["pluginResult"]["programReference"].is_object());
    let inventory = exported["pluginResult"]["mediaInventory"]
        .as_array()
        .expect("inventory");
    assert!(!inventory.is_empty());
    assert!(inventory.iter().all(|media| media["tapPoint"] == "program"));
    assert!(inventory.iter().all(|media| {
        media["recording"]["path"]
            .as_str()
            .is_some_and(|path| !path.is_empty())
    }));
    assert_eq!(exported["pluginResult"]["reconstruction"], "approximate");
    assert_eq!(
        exported["pluginResult"]["timebase"]["mediaClockDomain"],
        "engineRunPtsNs"
    );
    assert!(exported["pluginResult"]["timebase"]["mapping"].is_null());
    assert!(
        exported["pluginResult"]["issues"]
            .as_array()
            .expect("issues")
            .iter()
            .any(|issue| issue["kind"] == "missingClockMapping")
    );
}
