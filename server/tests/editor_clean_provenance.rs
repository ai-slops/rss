#![cfg(test)]

mod common;
mod editor_clean_provenance_support;

use std::{collections::BTreeSet, sync::Arc, time::Duration};

use common::Harness;
use editor_clean_provenance_support::{
    active_session_metadata, assert_provenance, export, free_udp_port, get_json, mixed_recording,
    reopen, srt_sender, wait_connected,
};
use gst::prelude::*;
use gstreamer as gst;
use live_production::plugins::EDITOR_PLUGIN_ID;
use serde_json::json;

#[tokio::test]
async fn clean_recording_reconnect_provenance_survives_catalog_reload() {
    // Given
    gst::init().expect("GStreamer");
    let harness = Harness::new();
    let cookie = harness.login().await;
    let initial = harness.snapshot(&cookie).await;
    let port = free_udp_port();
    let source = harness.command(&cookie, json!({
        "id": "clean-source", "kind": "source.create", "targetId": initial["project"]["id"],
        "expectedEngineRunId": initial["engineRunId"],
        "payload": {"name": "Clean SRT", "adapter": {"kind": "srt", "port": port, "bufferMs": 120}}
    })).await;
    assert_eq!(source["status"], "applied", "{source}");
    let configured = harness.snapshot(&cookie).await;
    let source_id = configured["project"]["sources"][0]["id"].clone();
    let video_track_id = configured["project"]["sources"][0]["tracks"][0]["id"].clone();
    let audio_track_id = configured["project"]["sources"][0]["tracks"][1]["id"].clone();
    let output = harness.command(&cookie, json!({
        "id": "clean-output", "kind": "output.create", "targetId": initial["project"]["id"],
        "expectedEngineRunId": initial["engineRunId"],
        "payload": {"name": "Clean archive", "adapter": {"kind": "sourceRecording", "sourceId": source_id, "segmentSeconds": 3}}
    })).await;
    assert_eq!(output["status"], "applied", "{output}");
    let ready = harness.snapshot(&cookie).await;
    let output_id = ready["project"]["outputs"][0]["id"].clone();
    for body in [
        json!({"id":"clean-editor-start","kind":"plugin.command","targetId":EDITOR_PLUGIN_ID,"expectedEngineRunId":initial["engineRunId"],"payload":{"command":"session.start","text":"clean-reconnect"}}),
        json!({"id":"clean-output-start","kind":"output.start","targetId":output_id,"expectedEngineRunId":initial["engineRunId"],"payload":{}}),
    ] {
        let response = harness.command(&cookie, body).await;
        assert_eq!(response["status"], "applied", "{response}");
    }
    let endpoint = get_json(
        &harness,
        &cookie,
        &format!(
            "/api/v1/sources/{}/publishing-endpoint",
            source_id.as_str().expect("source id")
        ),
    )
    .await;

    // When
    let first = srt_sender(port, endpoint["passphrase"].as_str().expect("passphrase"));
    first.set_state(gst::State::Playing).expect("first sender");
    wait_connected(&harness, &cookie).await;
    tokio::time::sleep(Duration::from_millis(1_200)).await;
    first
        .set_state(gst::State::Null)
        .expect("stop first sender");
    tokio::time::sleep(Duration::from_millis(500)).await;
    let second = srt_sender(port, endpoint["passphrase"].as_str().expect("passphrase"));
    second
        .set_state(gst::State::Playing)
        .expect("second sender");
    tokio::time::sleep(Duration::from_millis(2_500)).await;
    let active_metadata = active_session_metadata(
        harness._directory.path(),
        output_id.as_str().expect("output id"),
    );
    let active_generations = active_metadata["provenance"]
        .as_array()
        .expect("active provenance")
        .iter()
        .filter_map(|value| value["continuity_generation"].as_u64())
        .collect::<BTreeSet<_>>();
    assert_eq!(active_generations, BTreeSet::from([1, 2]));
    let stopped = harness
        .command(
            &cookie,
            json!({
                "id":"clean-output-stop","kind":"output.stop","targetId":output_id,
                "expectedEngineRunId":initial["engineRunId"],"payload":{}
            }),
        )
        .await;
    assert_eq!(stopped["status"], "applied", "{stopped}");
    second
        .set_state(gst::State::Null)
        .expect("stop second sender");
    let before_reload = export(&harness, &cookie, "clean-export-before", &initial).await;
    let directory = Arc::clone(&harness._directory);
    drop(harness);
    tokio::time::sleep(Duration::from_millis(500)).await;
    let reopened = reopen(directory);
    let reopened_cookie = reopened.login().await;
    let reopened_snapshot = reopened.snapshot(&reopened_cookie).await;
    let after_reload = export(
        &reopened,
        &reopened_cookie,
        "clean-export-after",
        &reopened_snapshot,
    )
    .await;

    // Then
    eprintln!(
        "persisted clean fragment: {}",
        mixed_recording(&after_reload)
    );
    assert_provenance(&before_reload, &source_id, &video_track_id, &audio_track_id);
    assert_provenance(&after_reload, &source_id, &video_track_id, &audio_track_id);
    assert_eq!(
        mixed_recording(&before_reload)["provenance"],
        mixed_recording(&after_reload)["provenance"]
    );
    assert_eq!(
        mixed_recording(&before_reload)["provenanceGaps"],
        mixed_recording(&after_reload)["provenanceGaps"]
    );
}
