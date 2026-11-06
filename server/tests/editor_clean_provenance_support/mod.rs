use std::{collections::BTreeSet, net::UdpSocket, sync::Arc, time::Duration};

use gst::prelude::*;
use gstreamer as gst;
use live_production::{
    coordinator::Coordinator,
    media_bridge::MediaBridge,
    plugins::EDITOR_PLUGIN_ID,
    routes::{self, AppState},
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

use crate::common::{Harness, request};

pub async fn export(harness: &Harness, cookie: &str, id: &str, snapshot: &Value) -> Value {
    let response=harness.command(cookie,json!({"id":id,"kind":"plugin.command","targetId":EDITOR_PLUGIN_ID,"expectedEngineRunId":snapshot["engineRunId"],"payload":{"command":"session.export"}})).await;
    assert_eq!(response["status"], "applied", "{response}");
    response["pluginResult"].clone()
}
pub async fn get_json(harness: &Harness, cookie: &str, uri: &str) -> Value {
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

pub fn assert_provenance(package: &Value, source: &Value, video: &Value, audio: &Value) {
    assert!(package["programReference"].is_null());
    let recording = mixed_recording(package);
    assert_eq!(recording["tapPoint"], "clean_source");
    assert_eq!(recording["mappingStatus"], "mapped");
    let file_start = decimal(&recording["mediaStartNs"]).expect("file start");
    let file_end = decimal(&recording["mediaEndNs"]).expect("file end");
    let intervals = recording["provenance"].as_array().expect("provenance");
    let generations = intervals
        .iter()
        .filter_map(|v| v["continuity_generation"].as_u64())
        .collect::<BTreeSet<_>>();
    assert_eq!(generations, BTreeSet::from([1, 2]));
    assert!(intervals.iter().all(|v| v["source_id"] == *source));
    for track in [video, audio] {
        let track_generations = intervals
            .iter()
            .filter(|value| value["track_id"] == *track)
            .filter_map(|value| value["continuity_generation"].as_u64())
            .collect::<BTreeSet<_>>();
        assert_eq!(track_generations, BTreeSet::from([1, 2]));
    }
    assert!(intervals.iter().all(
        |v| decimal(&v["media_start_ns"]).is_some_and(|s| s >= file_start)
            && decimal(&v["media_end_ns"]).is_some_and(|e| e <= file_end)
            && v["local_start_ns"].as_str().is_some()
            && v["local_end_ns"].as_str().is_some()
    ));
    let gaps = recording["provenanceGaps"].as_array().expect("gaps");
    assert!(!gaps.is_empty());
    assert!(gaps.iter().all(|g| {
        decimal(&g["local_start_ns"])
            .zip(decimal(&g["local_end_ns"]))
            .is_some_and(|(s, e)| e >= s)
            && decimal(&g["media_start_ns"])
                .zip(decimal(&g["media_end_ns"]))
                .is_some_and(|(s, e)| e >= s && s >= file_start && e <= file_end)
    }));
}
pub fn mixed_recording(package: &Value) -> &Value {
    package["mediaInventory"]
        .as_array()
        .expect("inventory")
        .iter()
        .map(|m| &m["recording"])
        .find(|r| {
            r["provenance"].as_array().is_some_and(|values| {
                values
                    .iter()
                    .filter_map(|v| v["continuity_generation"].as_u64())
                    .collect::<BTreeSet<_>>()
                    == BTreeSet::from([1, 2])
            })
        })
        .expect("mixed fragment")
}
fn decimal(value: &Value) -> Option<u64> {
    value.as_str()?.parse().ok()
}

pub fn active_session_metadata(state: &std::path::Path, output_id: &str) -> Value {
    let session = active_session_path(state, output_id);
    serde_json::from_slice(&std::fs::read(session.join("session.json")).expect("session metadata"))
        .expect("session JSON")
}
pub fn active_session_path(state: &std::path::Path, output_id: &str) -> std::path::PathBuf {
    let root = state.join("recordings").join(output_id);
    std::fs::read_dir(root)
        .expect("recording directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.join("session.json").is_file())
        .expect("active session")
}
pub fn preserve_native_session(session: &std::path::Path) {
    let Some(directory) = std::env::var_os("LIVE_PRODUCTION_PROVENANCE_EVIDENCE_DIR") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).expect("evidence directory");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
            .expect("private evidence directory");
    }
    for entry in std::fs::read_dir(session).expect("session files") {
        let entry = entry.expect("session entry");
        let path = entry.path();
        if entry.file_type().expect("session file type").is_file()
            && (entry.file_name() == "session.json"
                || path.extension().is_some_and(|value| value == "mkv"))
        {
            std::fs::copy(&path, directory.join(entry.file_name()))
                .expect("preserve native evidence");
        }
    }
}
pub async fn wait_connected(harness: &Harness, cookie: &str) {
    for _ in 0..100 {
        let snapshot = harness.snapshot(cookie).await;
        if snapshot["project"]["sources"][0]["tracks"][0]["connection"] == "connected" {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("SRT source did not connect")
}
pub fn reopen(directory: Arc<TempDir>) -> Harness {
    let media = MediaBridge::spawn().expect("reopened media");
    let coordinator =
        Coordinator::spawn(directory.path().join("project.sqlite"), media).expect("coordinator");
    let state = AppState::new("operator-password", false, coordinator);
    Harness {
        app: routes::router(state, directory.path().join("dist")),
        _directory: directory,
    }
}
pub fn free_udp_port() -> u16 {
    UdpSocket::bind(("127.0.0.1", 0))
        .expect("port")
        .local_addr()
        .expect("address")
        .port()
}
pub fn srt_sender(port: u16, passphrase: &str) -> gst::Pipeline {
    gst::parse::launch(&format!("mpegtsmux name=mux alignment=7 ! srtsink uri=\"srt://127.0.0.1:{port}?mode=caller&passphrase={passphrase}&pbkeylen=16\" wait-for-connection=true videotestsrc is-live=true pattern=ball ! video/x-raw,width=640,height=360,framerate=30/1 ! videoconvert ! x264enc tune=zerolatency key-int-max=30 bitrate=1000 ! h264parse config-interval=-1 ! mux. audiotestsrc is-live=true wave=ticks ! audioconvert ! audioresample ! audio/x-raw,rate=48000,channels=2 ! voaacenc bitrate=128000 ! aacparse ! mux.")).expect("sender").downcast::<gst::Pipeline>().expect("pipeline")
}
