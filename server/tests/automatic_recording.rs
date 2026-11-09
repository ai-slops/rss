use live_production::{coordinator::Coordinator, media_bridge::MediaBridge};
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn records_without_browser_and_resumes_same_archive_after_restart() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("project.sqlite");
    let media = MediaBridge::spawn().unwrap();
    let coordinator = Coordinator::spawn(database.clone(), media).unwrap();
    let initial = coordinator.snapshot().await.unwrap();
    let command = serde_json::from_value(json!({
        "id": "automatic-source", "kind": "source.create", "targetId": initial.project.id,
        "expectedEngineRunId": initial.engine_run_id,
        "payload": {"name": "Camera archive", "adapter": {"kind": "synthetic", "pattern": "ball", "frequencyHz": 550}}
    })).unwrap();
    let result = coordinator.execute(command).await.unwrap();
    assert_eq!(result["status"], "applied", "{result}");
    // No snapshots or browser traffic while media arrives.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let configured = coordinator.snapshot().await.unwrap();
    assert!(configured.project.sources[0].record_on_connect);
    assert_eq!(configured.project.outputs.len(), 1);
    assert!(configured.project.outputs[0].automatic);
    let archive_id = configured.project.outputs[0].id.clone();
    coordinator.shutdown().await.unwrap();
    let files_before = recording_files(directory.path());
    assert!(
        !files_before.is_empty(),
        "automatic source recording produced no files"
    );

    let media = MediaBridge::spawn().unwrap();
    let resumed = Coordinator::spawn(database, media).unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;
    let snapshot = resumed.snapshot().await.unwrap();
    assert_eq!(snapshot.project.outputs.len(), 1);
    assert_eq!(snapshot.project.outputs[0].id, archive_id);
    assert_eq!(snapshot.output_statuses[0].desired, "running");
    resumed.shutdown().await.unwrap();
    let files_after = recording_files(directory.path());
    assert!(files_after.len() > files_before.len());
    for file in &files_after {
        let status = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-xerror", "-i"])
            .arg(file)
            .args([
                "-fps_mode",
                "passthrough",
                "-enc_time_base",
                "demux",
                "-f",
                "null",
                "-",
            ])
            .status()
            .unwrap();
        assert!(status.success(), "failed to decode {}", file.display());
    }
}

fn recording_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(recording_files(&path));
        } else if path.extension().is_some_and(|extension| extension == "mkv") {
            files.push(path);
        }
    }
    files
}

mod common;
#[allow(dead_code)]
mod editor_clean_provenance_support;

#[tokio::test]
async fn idle_input_stays_armed_and_reconnect_finalizes_separate_sessions() {
    use common::Harness;
    use editor_clean_provenance_support::{free_udp_port, get_json, srt_sender_at_fps};
    use gstreamer::{self as gst, prelude::*};

    gst::init().unwrap();
    let harness = Harness::new();
    let cookie = harness.login().await;
    let initial = harness.snapshot(&cookie).await;
    let port = free_udp_port();
    let created = harness.command(&cookie, json!({
        "id":"auto-srt-create", "kind":"source.create", "targetId":initial["project"]["id"],
        "expectedEngineRunId": initial["engineRunId"],
        "payload":{"name":"Automatic SRT", "adapter":{"kind":"srt","port":port,"bufferMs":120}}
    })).await;
    assert_eq!(created["status"], "applied", "{created}");
    let configured = harness.snapshot(&cookie).await;
    let source_id = configured["project"]["sources"][0]["id"].as_str().unwrap();
    let endpoint = get_json(
        &harness,
        &cookie,
        &format!("/api/v1/sources/{source_id}/publishing-endpoint"),
    )
    .await;
    let audio_track = configured["project"]["sources"][0]["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|track| track["kind"] == "audio")
        .unwrap()["id"]
        .clone();
    let routed = harness.command(&cookie, json!({
        "id":"auto-audio-route", "kind":"audio.apply", "targetId": initial["project"]["id"],
        "expectedEngineRunId": initial["engineRunId"], "payload":{"routes":[{"trackId":audio_track,"gainDb":0,"muted":false,"routed":true}]}
    })).await;
    assert_eq!(routed["status"], "applied", "{routed}");
    tokio::time::sleep(Duration::from_secs(6)).await;
    let idle = harness.snapshot(&cookie).await;
    assert_eq!(idle["outputStatuses"][0]["desired"], "running");
    assert_ne!(idle["outputStatuses"][0]["actual"], "failed");
    for _ in 0..2 {
        let sender = srt_sender_at_fps(port, endpoint["passphrase"].as_str().unwrap(), 60);
        sender.set_state(gst::State::Playing).unwrap();
        tokio::time::sleep(Duration::from_secs(4)).await;
        let audible = harness.snapshot(&cookie).await;
        assert!(
            audible["project"]["audioRoutes"][0]["inputPeakDb"]
                .as_f64()
                .is_some_and(|level| level > -60.0),
            "live Program route must contain the received source audio: {audible}"
        );
        sender.set_state(gst::State::Null).unwrap();
        // No browser polling while the server detects loss and closes the file.
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
    let archived = harness.snapshot(&cookie).await;
    let recordings = archived["recordings"].as_array().unwrap();
    assert!(recordings.len() >= 2, "{archived}");
    let sessions: std::collections::HashSet<_> = recordings
        .iter()
        .map(|recording| recording["sessionId"].as_str().unwrap())
        .collect();
    assert_eq!(sessions.len(), 2, "{recordings:?}");
    for recording in recordings {
        assert_eq!(recording["validation"], "decoded", "{recording}");
        assert_eq!(recording["tapPoint"], "clean_source");
        assert!(recording["endNs"].is_string());
        let decoded = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-xerror",
                "-i",
                recording["path"].as_str().unwrap(),
                "-fps_mode",
                "passthrough",
                "-enc_time_base",
                "demux",
                "-f",
                "null",
                "-",
            ])
            .status()
            .unwrap();
        assert!(decoded.success());
    }
    let stop = harness.command(&cookie, json!({
        "id":"auto-stop", "kind":"output.stop", "targetId": configured["project"]["outputs"][0]["id"],
        "expectedEngineRunId": initial["engineRunId"], "payload":{}
    })).await;
    assert_ne!(
        stop["status"], "applied",
        "automatic capture must stay enabled"
    );
    let unrouted = harness.command(&cookie, json!({
        "id":"auto-audio-unroute", "kind":"audio.apply", "targetId": initial["project"]["id"],
        "expectedEngineRunId": initial["engineRunId"], "payload":{"routes":[]}
    })).await;
    assert_eq!(unrouted["status"], "applied", "{unrouted}");
    let files = recording_files(harness._directory.path());
    let deleted = harness
        .command(
            &cookie,
            json!({
                "id":"auto-source-delete", "kind":"source.delete", "targetId": source_id,
                "expectedEngineRunId": initial["engineRunId"], "payload":{}
            }),
        )
        .await;
    assert_eq!(deleted["status"], "applied", "{deleted}");
    assert!(
        files.iter().all(|file| file.is_file()),
        "removing inputs must preserve archived media"
    );
}
