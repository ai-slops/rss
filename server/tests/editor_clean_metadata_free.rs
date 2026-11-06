#![cfg(test)]

mod common;

use std::{
    collections::{BTreeMap, BTreeSet},
    net::UdpSocket,
    time::Duration,
};

use common::{Harness, request};
use gst::prelude::*;
use gstreamer as gst;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn black_silent_reconnect_records_exact_generations_without_consumer_metadata() {
    // Given
    gst::init().expect("GStreamer");
    let harness = Harness::new();
    let cookie = harness.login().await;
    let initial = harness.snapshot(&cookie).await;
    let port = free_udp_port();
    let source = harness
        .command(
            &cookie,
            json!({
                "id":"metadata-free-source","kind":"source.create","targetId":initial["project"]["id"],
                "expectedEngineRunId":initial["engineRunId"],
                "payload":{"name":"Metadata-free SRT","adapter":{"kind":"srt","port":port,"bufferMs":120}}
            }),
        )
        .await;
    assert_eq!(source["status"], "applied", "{source}");
    let configured = harness.snapshot(&cookie).await;
    let source_id = configured["project"]["sources"][0]["id"].clone();
    let video_track_id = configured["project"]["sources"][0]["tracks"][0]["id"].clone();
    let audio_track_id = configured["project"]["sources"][0]["tracks"][1]["id"].clone();
    let output = harness
        .command(
            &cookie,
            json!({
                "id":"metadata-free-output","kind":"output.create","targetId":initial["project"]["id"],
                "expectedEngineRunId":initial["engineRunId"],
                "payload":{"name":"Metadata-free archive","adapter":{"kind":"sourceRecording","sourceId":source_id,"segmentSeconds":3}}
            }),
        )
        .await;
    assert_eq!(output["status"], "applied", "{output}");
    let ready = harness.snapshot(&cookie).await;
    let output_id = ready["project"]["outputs"][0]["id"].clone();
    let started = harness
        .command(
            &cookie,
            json!({
                "id":"metadata-free-start","kind":"output.start","targetId":output_id,
                "expectedEngineRunId":initial["engineRunId"],"payload":{}
            }),
        )
        .await;
    assert_eq!(started["status"], "applied", "{started}");
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
    let first = black_silent_sender(port, endpoint["passphrase"].as_str().expect("passphrase"));
    first.set_state(gst::State::Playing).expect("first sender");
    wait_connected(&harness, &cookie).await;
    tokio::time::sleep(Duration::from_millis(1_200)).await;
    first
        .set_state(gst::State::Null)
        .expect("stop first sender");
    tokio::time::sleep(Duration::from_millis(500)).await;
    let second = black_silent_sender(port, endpoint["passphrase"].as_str().expect("passphrase"));
    second
        .set_state(gst::State::Playing)
        .expect("second sender");
    tokio::time::sleep(Duration::from_millis(2_500)).await;
    let metadata = active_session_metadata(
        harness._directory.path(),
        output_id.as_str().expect("output id"),
    );
    let stopped = harness
        .command(
            &cookie,
            json!({
                "id":"metadata-free-stop","kind":"output.stop","targetId":output_id,
                "expectedEngineRunId":initial["engineRunId"],"payload":{}
            }),
        )
        .await;
    second
        .set_state(gst::State::Null)
        .expect("stop second sender");
    drop(first);
    drop(second);
    tokio::time::sleep(Duration::from_millis(250)).await;

    // Then
    assert_eq!(stopped["status"], "applied", "{stopped}");
    let generations = metadata["provenance"]
        .as_array()
        .expect("provenance")
        .iter()
        .fold(
            BTreeMap::<String, BTreeSet<u64>>::new(),
            |mut values, value| {
                assert_eq!(value["source_id"], source_id);
                values
                    .entry(value["track_id"].as_str().expect("track id").to_owned())
                    .or_default()
                    .insert(value["continuity_generation"].as_u64().expect("generation"));
                values
            },
        );
    assert_eq!(
        generations.get(video_track_id.as_str().expect("video track")),
        Some(&BTreeSet::from([1, 2]))
    );
    assert_eq!(
        generations.get(audio_track_id.as_str().expect("audio track")),
        Some(&BTreeSet::from([1, 2]))
    );
}

async fn get_json(harness: &Harness, cookie: &str, uri: &str) -> serde_json::Value {
    let response = harness
        .app
        .clone()
        .oneshot(request("GET", uri, Some(cookie), None))
        .await
        .expect("GET response");
    let bytes = axum::body::to_bytes(response.into_body(), 1_048_576)
        .await
        .expect("GET body");
    serde_json::from_slice(&bytes).expect("GET JSON")
}

async fn wait_connected(harness: &Harness, cookie: &str) {
    for _ in 0..100 {
        let snapshot = harness.snapshot(cookie).await;
        if snapshot["project"]["sources"][0]["tracks"][0]["connection"] == "connected" {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("SRT source did not connect")
}

fn free_udp_port() -> u16 {
    UdpSocket::bind(("127.0.0.1", 0))
        .expect("port")
        .local_addr()
        .expect("address")
        .port()
}

fn active_session_metadata(state: &std::path::Path, output_id: &str) -> serde_json::Value {
    let root = state.join("recordings").join(output_id);
    let session = std::fs::read_dir(root)
        .expect("recording directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.join("session.json").is_file())
        .expect("active session");
    serde_json::from_slice(&std::fs::read(session.join("session.json")).expect("session metadata"))
        .expect("session JSON")
}

fn black_silent_sender(port: u16, passphrase: &str) -> gst::Pipeline {
    gst::parse::launch(&format!(
        "mpegtsmux name=mux alignment=7 ! srtsink uri=\"srt://127.0.0.1:{port}?mode=caller&passphrase={passphrase}&pbkeylen=16\" wait-for-connection=true \
         videotestsrc is-live=true pattern=black ! video/x-raw,width=640,height=360,framerate=30/1 ! videoconvert ! x264enc tune=zerolatency key-int-max=30 bitrate=1000 ! h264parse config-interval=-1 ! mux. \
         audiotestsrc is-live=true wave=silence ! audioconvert ! audioresample ! audio/x-raw,rate=48000,channels=2 ! voaacenc bitrate=128000 ! aacparse ! mux."
    ))
    .expect("sender")
    .downcast::<gst::Pipeline>()
    .expect("pipeline")
}
