mod common;

use common::{Harness, request};
use live_production::{
    command::Command,
    coordinator::Coordinator,
    media_bridge::MediaBridge,
    plugins::{EDITOR_PLUGIN_ID, EXTERNAL_EVENT_PLUGIN_ID},
};
use serde_json::{Value, json};
use tokio_stream::StreamExt;
use tower::ServiceExt;

fn command(value: Value) -> Command {
    serde_json::from_value(value).expect("valid command fixture")
}

#[tokio::test]
async fn builtin_media_factories_are_registered_and_used_by_public_commands() {
    // Given
    assert!(
        live_production::plugins::SOURCE_FACTORY_KINDS
            .contains(&live_production::plugins::SourceFactoryKind::Synthetic)
    );
    assert!(
        live_production::plugins::OUTPUT_FACTORY_KINDS
            .contains(&live_production::plugins::OutputFactoryKind::ProgramRecording)
    );
    let directory = tempfile::tempdir().expect("temporary state directory");
    let coordinator = Coordinator::spawn(
        directory.path().join("project.sqlite"),
        MediaBridge::spawn().expect("media engine"),
    )
    .expect("coordinator");
    let initial = coordinator.snapshot().await.expect("initial snapshot");

    // When
    let source = coordinator.execute(command(json!({
        "id": "factory-source", "kind": "source.create", "targetId": initial.project.id,
        "expectedEngineRunId": initial.engine_run_id,
        "payload": {"name": "Factory camera", "adapter": {"kind": "synthetic", "pattern": "ball", "frequencyHz": 440}}
    }))).await.expect("source command");
    let output = coordinator.execute(command(json!({
        "id": "factory-output", "kind": "output.create", "targetId": initial.project.id,
        "expectedEngineRunId": initial.engine_run_id,
        "payload": {"name": "Factory Program", "adapter": {"kind": "programRecording", "segmentSeconds": 1}}
    }))).await.expect("output command");

    // Then
    assert_eq!(source["status"], "applied", "{source}");
    assert_eq!(output["status"], "applied", "{output}");
    let snapshot = coordinator.snapshot().await.expect("factory snapshot");
    assert_eq!(snapshot.project.sources.len(), 1);
    assert_eq!(snapshot.project.outputs.len(), 1);
}

#[tokio::test]
async fn trusted_plugins_register_and_external_event_uses_public_command() {
    // Given
    let directory = tempfile::tempdir().expect("temporary state directory");
    let coordinator = Coordinator::spawn(
        directory.path().join("project.sqlite"),
        MediaBridge::spawn().expect("media engine"),
    )
    .expect("coordinator");
    let initial = coordinator.snapshot().await.expect("initial snapshot");

    // When
    let response = coordinator
        .execute(command(json!({
            "id": "external-event-1", "kind": "plugin.command", "targetId": EXTERNAL_EVENT_PLUGIN_ID,
            "expectedEngineRunId": initial.engine_run_id,
            "payload": {"command": "event.record", "text": "Goal"}
        })))
        .await
        .expect("plugin command");

    // Then
    assert_eq!(response["status"], "applied");
    assert_eq!(response["pluginResult"]["eventCount"], 1);
    let snapshot = coordinator.snapshot().await.expect("current snapshot");
    assert!(
        snapshot
            .plugins
            .iter()
            .any(|plugin| { plugin.id == EDITOR_PLUGIN_ID && plugin.available && !plugin.enabled })
    );
    let panel = snapshot
        .plugins
        .iter()
        .find(|plugin| plugin.id == EXTERNAL_EVENT_PLUGIN_ID)
        .expect("external event registration");
    assert!(panel.available && panel.enabled);
    assert_eq!(panel.panel.as_ref().expect("panel").kind, "externalEvent");
    assert!(
        panel
            .commands
            .iter()
            .any(|command| { command.name == "event.record" && command.text == "required" })
    );
    let events = coordinator.events("0").await.expect("plugin observation");
    assert!(
        !serde_json::to_string(&events.events)
            .expect("serialize observations")
            .contains("Goal")
    );
}

#[tokio::test]
async fn applied_events_include_consistent_snapshots_after_cursor() {
    // Given
    let directory = tempfile::tempdir().expect("temporary state directory");
    let coordinator = Coordinator::spawn(
        directory.path().join("project.sqlite"),
        MediaBridge::spawn().expect("media engine"),
    )
    .expect("coordinator");
    let initial = coordinator.snapshot().await.expect("initial snapshot");
    coordinator
        .execute(command(json!({
            "id": "scene-create-observed", "kind": "scene.create", "targetId": initial.project.id,
            "expectedEngineRunId": initial.engine_run_id, "payload": {"name": "Observed"}
        })))
        .await
        .expect("create scene");

    // When
    let page = coordinator
        .events(&initial.cursor)
        .await
        .expect("event page");

    // Then
    assert!(page.gap.is_none());
    assert_eq!(page.events.len(), 1);
    let event = &page.events[0];
    assert_eq!(event.kind, "production.applied");
    assert_eq!(event.data["snapshot"]["cursor"], event.cursor);
    assert_eq!(
        event.data["snapshot"]["project"]["scenes"]
            .as_array()
            .expect("scenes")
            .len(),
        2
    );
    assert!(event.data["snapshot"]["program"]["appliedScene"].is_object());
}

#[tokio::test]
async fn sse_replays_real_event_after_requested_cursor() {
    // Given
    let harness = Harness::new();
    let cookie = harness.login().await;
    let initial = harness.snapshot(&cookie).await;
    let response = harness
        .command(
            &cookie,
            json!({
                "id": "sse-observed", "kind": "scene.create", "targetId": initial["project"]["id"],
                "expectedEngineRunId": initial["engineRunId"], "payload": {"name": "SSE observed"}
            }),
        )
        .await;
    assert_eq!(response["status"], "applied");

    // When
    let response = harness
        .app
        .clone()
        .oneshot(request(
            "GET",
            &format!(
                "/api/v1/events?after={}",
                initial["cursor"].as_str().expect("snapshot cursor")
            ),
            Some(&cookie),
            None,
        ))
        .await
        .expect("SSE response");
    let mut stream = response.into_body().into_data_stream();
    let bytes = tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
        .await
        .expect("SSE frame timeout")
        .expect("SSE frame")
        .expect("SSE bytes");
    let frame = String::from_utf8(bytes.to_vec()).expect("UTF-8 SSE frame");

    // Then
    assert!(frame.contains("event: production.applied"), "{frame}");
    assert!(frame.contains("id: 1"), "{frame}");
    assert!(frame.contains("SSE observed"), "{frame}");
}
