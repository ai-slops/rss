use std::time::{Duration, Instant};

use gst::prelude::*;
use gstreamer as gst;
use live_production_media::output::{
    DestinationUrl, MediaTap, OutputDefinition, OutputId, OutputManager, OutputState,
    RecordingDefinition,
};

#[test]
fn recording_owner_finalizes_decodable_bounded_segments() -> anyhow::Result<()> {
    // Given
    gst::init()?;
    let directory = tempfile::tempdir()?;
    let channels = MediaTap::new("recording-test-video", "recording-test-audio")?;
    let producer = producer(&channels)?;
    producer.set_state(gst::State::Playing)?;
    let manager = OutputManager::new();
    let output_id = OutputId::new("program-recording")?;
    manager.register(OutputDefinition::Recording(RecordingDefinition::new(
        output_id.clone(),
        channels,
        directory.path(),
        Duration::from_secs(1),
    )?))?;

    // When
    manager.start(&output_id)?;
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
    producer.set_state(gst::State::Null)?;
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

#[test]
fn duplicate_start_keeps_single_recording_owner() -> anyhow::Result<()> {
    // Given
    gst::init()?;
    let directory = tempfile::tempdir()?;
    let channels = MediaTap::new("duplicate-test-video", "duplicate-test-audio")?;
    let producer = producer(&channels)?;
    producer.set_state(gst::State::Playing)?;
    let manager = OutputManager::new();
    let output_id = OutputId::new("duplicate-recorder")?;
    manager.register(OutputDefinition::Recording(RecordingDefinition::new(
        output_id.clone(),
        channels,
        directory.path(),
        Duration::from_secs(1),
    )?))?;
    manager.start(&output_id)?;
    wait_for_media(&manager, &output_id)?;

    // When
    manager.start(&output_id)?;
    std::thread::sleep(Duration::from_secs(2));
    manager.stop(&output_id)?;

    // Then
    assert_eq!(std::fs::read_dir(directory.path())?.count(), 1);
    assert!(!manager.recordings(&output_id)?.is_empty());
    producer.set_state(gst::State::Null)?;
    Ok(())
}

#[test]
fn manager_drop_drains_and_joins_active_recorder() -> anyhow::Result<()> {
    // Given
    gst::init()?;
    let directory = tempfile::tempdir()?;
    let channels = MediaTap::new("shutdown-test-video", "shutdown-test-audio")?;
    let producer = producer(&channels)?;
    producer.set_state(gst::State::Playing)?;
    let output_id = OutputId::new("shutdown-recorder")?;
    {
        let manager = OutputManager::new();
        manager.register(OutputDefinition::Recording(RecordingDefinition::new(
            output_id.clone(),
            channels,
            directory.path(),
            Duration::from_secs(1),
        )?))?;
        manager.start(&output_id)?;
        wait_for_media(&manager, &output_id)?;
        std::thread::sleep(Duration::from_secs(2));

        // When
        drop(manager);
    }

    // Then
    let session = std::fs::read_dir(directory.path())?
        .next()
        .transpose()?
        .ok_or_else(|| anyhow::anyhow!("missing recording session"))?
        .path();
    let catalog: serde_json::Value =
        serde_json::from_slice(&std::fs::read(session.join("catalog.json"))?)?;
    assert!(
        catalog
            .as_array()
            .is_some_and(|segments| !segments.is_empty())
    );
    producer.set_state(gst::State::Null)?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn recorder_failure_preserves_closed_and_partial_catalog_entries() -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    // Given
    gst::init()?;
    let directory = tempfile::tempdir()?;
    let channels = MediaTap::new("partial-test-video", "partial-test-audio")?;
    let producer = producer(&channels)?;
    producer.set_state(gst::State::Playing)?;
    let manager = OutputManager::new();
    let output_id = OutputId::new("partial-recorder")?;
    manager.register(OutputDefinition::Recording(RecordingDefinition::new(
        output_id.clone(),
        channels,
        directory.path(),
        Duration::from_secs(1),
    )?))?;
    manager.start(&output_id)?;
    wait_for_media(&manager, &output_id)?;
    std::thread::sleep(Duration::from_secs(2));
    let session = std::fs::read_dir(directory.path())?
        .next()
        .transpose()?
        .ok_or_else(|| anyhow::anyhow!("missing recording session"))?
        .path();

    // When
    std::fs::set_permissions(&session, std::fs::Permissions::from_mode(0o500))?;
    wait_for_state(&manager, &output_id, OutputState::Failed)?;

    // Then
    let recordings = manager.recordings(&output_id)?;
    assert!(!recordings.is_empty());
    assert!(recordings.iter().all(|segment| segment.bytes > 0));
    std::fs::set_permissions(&session, std::fs::Permissions::from_mode(0o700))?;
    producer.set_state(gst::State::Null)?;
    Ok(())
}

fn producer(tap: &MediaTap) -> anyhow::Result<gst::Pipeline> {
    let description = format!(
        "videotestsrc is-live=true pattern=ball ! video/x-raw,width=320,height=180,framerate=30/1 ! intervideosink channel={} sync=false \
         audiotestsrc is-live=true wave=ticks ! audio/x-raw,rate=48000,channels=2 ! interaudiosink channel={} sync=false",
        tap.video_channel(),
        tap.audio_channel()
    );
    gst::parse::launch(&description)?
        .downcast::<gst::Pipeline>()
        .map_err(|_| anyhow::anyhow!("producer description did not create a pipeline"))
}

fn wait_for_state(
    manager: &OutputManager,
    id: &OutputId,
    state: OutputState,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if manager.snapshot(id)?.actual == state {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    anyhow::bail!(
        "output did not reach {state:?}: {:?}",
        manager.snapshot(id)?
    )
}

fn wait_for_media(manager: &OutputManager, id: &OutputId) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if manager.snapshot(id)?.last_media_at_ns.is_some() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    anyhow::bail!("output produced no media")
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
