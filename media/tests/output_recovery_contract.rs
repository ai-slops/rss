mod output_support;

use std::time::Duration;

use gst::prelude::*;
use gstreamer as gst;
use live_production_media::output::{
    MediaTap, OutputDefinition, OutputId, OutputManager, OutputState, RecordingDefinition,
};
use output_support::{producer, wait_for_media, wait_for_state};

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
