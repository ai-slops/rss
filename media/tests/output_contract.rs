mod output_state_support;
mod output_support;

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gst::prelude::*;
use gstreamer as gst;
use live_production_media::output::{
    DestinationUrl, MediaTap, OutputDefinition, OutputId, OutputManager, OutputState,
    RecordingDefinition,
};
use live_production_media::protocol::{EngineClockMapping, EngineRunId, Nanoseconds};
use output_state_support::wait_for_state;
use output_support::{producer, wait_for_media};

#[test]
fn recording_owner_finalizes_decodable_bounded_segments() -> anyhow::Result<()> {
    // Given
    gst::init()?;
    let directory = tempfile::tempdir()?;
    let channels = MediaTap::new("recording-test-video", "recording-test-audio")?;
    let producer = producer(&channels)?;
    let reference = gst::Caps::builder("timestamp/x-live-production-engine")
        .field("engine-run-id", "recording-test-run")
        .build();
    producer
        .by_name("fixture_video_tap")
        .and_then(|tap| tap.static_pad("sink"))
        .ok_or_else(|| anyhow::anyhow!("fixture video tap"))?
        .add_probe(gst::PadProbeType::BUFFER, move |_, info| {
            if let Some(buffer) = info.buffer_mut()
                && let Some(pts) = buffer.pts()
            {
                let time = gst::ClockTime::from_nseconds(5_000_000_000 + pts.nseconds());
                gst::ReferenceTimestampMeta::add(buffer.make_mut(), &reference, time, None);
            }
            gst::PadProbeReturn::Ok
        })
        .ok_or_else(|| anyhow::anyhow!("fixture timestamp observation"))?;
    producer.set_state(gst::State::Playing)?;
    let manager = OutputManager::new();
    let output_id = OutputId::new("program-recording")?;
    manager.register(OutputDefinition::Recording(
        RecordingDefinition::new(
            output_id.clone(),
            channels,
            directory.path(),
            Duration::from_secs(1),
        )?
        .with_engine_run_id(&EngineRunId::new("recording-test-run")?),
    ))?;

    // When
    let wall_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_nanos()
        .try_into()?;
    manager.start_with_clock(
        &output_id,
        Some(EngineClockMapping {
            media_time: Nanoseconds::new(5_000_000_000),
            wall_time: Nanoseconds::new(wall_time),
            uncertainty: Nanoseconds::new(1_000_000),
        }),
    )?;
    wait_for_state(&manager, &output_id, OutputState::Running)?;
    wait_for_media(&manager, &output_id)?;
    std::thread::sleep(Duration::from_secs(3));
    manager.stop(&output_id)?;
    wait_for_state(&manager, &output_id, OutputState::Stopped)?;

    // Then
    let recordings = manager.recordings(&output_id)?;
    assert!(
        recordings.len() >= 2,
        "expected bounded segments: {recordings:?}"
    );
    assert!(recordings.iter().all(|segment| segment.bytes > 0));
    assert!(recordings.iter().all(|segment| segment.decoded));
    assert!(recordings.iter().all(|segment| segment.sha256.len() == 64));
    assert!(
        recordings.iter().all(|segment| {
            segment.engine_run_id.as_deref() == Some("recording-test-run")
                && segment.media_start_ns.is_some()
                && segment.media_end_ns > segment.media_start_ns
                && segment.media_uncertainty_ns.is_some()
        }),
        "{recordings:#?}"
    );
    let distinct_starts = recordings
        .iter()
        .filter_map(|segment| segment.media_start_ns)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(distinct_starts.len(), recordings.len());
    producer.set_state(gst::State::Null)?;
    Ok(())
}

#[test]
fn legacy_recording_catalog_loads_with_unknown_media_mapping() -> anyhow::Result<()> {
    // Given
    let directory = tempfile::tempdir()?;
    let session = directory.path().join("legacy-session");
    std::fs::create_dir(&session)?;
    std::fs::write(
        session.join("catalog.json"),
        serde_json::to_vec(&serde_json::json!([{
            "path": session.join("segment-00000.mkv"),
            "start_ns": 100,
            "end_ns": 200,
            "bytes": 10,
            "sha256": "legacy",
            "decoded": true
        }]))?,
    )?;
    let manager = OutputManager::new();
    let output_id = OutputId::new("legacy-recording")?;
    manager.register(OutputDefinition::Recording(RecordingDefinition::new(
        output_id.clone(),
        MediaTap::new("legacy-video", "legacy-audio")?,
        directory.path(),
        Duration::from_secs(1),
    )?))?;

    // When
    let recordings = manager.recordings(&output_id)?;

    // Then
    assert_eq!(recordings.len(), 1);
    assert!(recordings[0].engine_run_id.is_none());
    assert!(recordings[0].media_start_ns.is_none());
    assert!(recordings[0].media_end_ns.is_none());
    assert!(recordings[0].media_uncertainty_ns.is_none());
    Ok(())
}

#[test]
fn manual_stop_cancels_retry_without_affecting_recording() -> anyhow::Result<()> {
    // Given
    gst::init()?;
    let directory = tempfile::tempdir()?;
    let channels = MediaTap::new("independent-test-video", "independent-test-audio")?;
    let producer = producer(&channels)?;
    producer.set_state(gst::State::Playing)?;
    let manager = OutputManager::new();
    let recording_id = OutputId::new("independent-recording")?;
    let network_id = OutputId::new("failed-network")?;
    manager.register(OutputDefinition::Recording(RecordingDefinition::new(
        recording_id.clone(),
        channels.clone(),
        directory.path(),
        Duration::from_secs(1),
    )?))?;
    manager.register(OutputDefinition::rtmps(
        network_id.clone(),
        channels,
        DestinationUrl::new("rtmps://127.0.0.1:1/live/secret")?,
    ))?;
    manager.start(&recording_id)?;
    manager.start(&network_id)?;
    wait_for_state(&manager, &recording_id, OutputState::Running)?;
    wait_for_retry(&manager, &network_id)?;

    // When
    manager.stop(&network_id)?;
    wait_for_state(&manager, &network_id, OutputState::Stopped)?;

    // Then
    assert_eq!(manager.snapshot(&network_id)?.desired, OutputState::Stopped);
    assert_eq!(
        manager.snapshot(&recording_id)?.actual,
        OutputState::Running
    );
    manager.stop(&recording_id)?;
    wait_for_state(&manager, &recording_id, OutputState::Stopped)?;
    assert!(!manager.recordings(&recording_id)?.is_empty());
    producer.set_state(gst::State::Null)?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn storage_failure_does_not_stop_an_independent_recorder() -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    // Given
    gst::init()?;
    let directory = tempfile::tempdir()?;
    let blocked = directory.path().join("blocked");
    let healthy = directory.path().join("healthy");
    std::fs::create_dir(&blocked)?;
    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o500))?;
    let channels = MediaTap::new("storage-test-video", "storage-test-audio")?;
    let producer = producer(&channels)?;
    producer.set_state(gst::State::Playing)?;
    let manager = OutputManager::new();
    let blocked_id = OutputId::new("blocked-recorder")?;
    let healthy_id = OutputId::new("healthy-recorder")?;
    manager.register(OutputDefinition::Recording(RecordingDefinition::new(
        blocked_id.clone(),
        channels.clone(),
        &blocked,
        Duration::from_secs(1),
    )?))?;
    manager.register(OutputDefinition::Recording(RecordingDefinition::new(
        healthy_id.clone(),
        channels,
        &healthy,
        Duration::from_secs(1),
    )?))?;

    // When
    let blocked_start = manager.start(&blocked_id);
    manager.start(&healthy_id)?;
    wait_for_state(&manager, &healthy_id, OutputState::Running)?;
    std::thread::sleep(Duration::from_secs(2));

    // Then
    assert!(blocked_start.is_err());
    assert_eq!(manager.snapshot(&blocked_id)?.actual, OutputState::Failed);
    assert_eq!(manager.snapshot(&healthy_id)?.actual, OutputState::Running);
    manager.stop(&healthy_id)?;
    assert!(!manager.recordings(&healthy_id)?.is_empty());
    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o700))?;
    producer.set_state(gst::State::Null)?;
    Ok(())
}

fn wait_for_retry(manager: &OutputManager, id: &OutputId) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let state = manager.snapshot(id)?.actual;
        if matches!(state, OutputState::Retrying | OutputState::Starting) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    anyhow::bail!("network output did not enter retry lifecycle")
}
