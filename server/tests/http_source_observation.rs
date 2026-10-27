#![cfg(test)]

mod common;

use common::Harness;
use serde_json::json;
use std::time::{Duration, Instant};

#[tokio::test]
async fn operator_snapshot_exposes_measured_drift_with_precise_time_units() {
    let harness = Harness::new();
    let cookie = harness.login().await;
    let initial = harness.snapshot(&cookie).await;
    let created = harness.command(&cookie, json!({
        "id": "observed-source", "kind": "source.create", "targetId": initial["project"]["id"],
        "expectedEngineRunId": initial["engineRunId"],
        "payload": {"name": "Observed source", "adapter": {"kind": "synthetic", "pattern": "ball", "frequencyHz": 440}}
    })).await;
    assert_eq!(created["status"], "applied", "{created}");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let snapshot = harness.snapshot(&cookie).await;
        let tracks = snapshot["project"]["sources"][0]["tracks"]
            .as_array()
            .expect("tracks");
        if tracks.iter().all(|track| track["drift"].is_object()) {
            for track in tracks {
                let drift = &track["drift"];
                assert!(drift["arrivalSkewNs"].as_str().is_some());
                assert!(drift["observationWindowNanoseconds"].as_str().is_some());
                assert!(drift["timestampResidualRms"].as_str().is_some());
                assert!(drift["rateStandardErrorPpm"].as_u64().is_some());
                assert!(drift["samples"].as_str().is_some());
            }
            break;
        }
        assert!(
            Instant::now() < deadline,
            "measured track drift never reached operator snapshot"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
