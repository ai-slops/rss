mod output_recovery_support;
mod output_state_support;
mod output_support;

use std::time::Duration;

use gst::prelude::*;
use gstreamer as gst;
use live_production_media::output::{
    MediaTap, OutputDefinition, OutputId, OutputManager, OutputState, RecordingDefinition,
    RecordingRecovery,
};
use output_recovery_support::{fragment_hashes, recorded_fixture, reload_recordings};
use output_state_support::wait_for_state;
use output_support::{producer, wait_for_media};

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

#[test]
fn startup_discovers_fragments_when_catalog_is_missing() -> anyhow::Result<()> {
    // Given
    let fixture = recorded_fixture("missing-catalog")?;
    let expected = fragment_hashes(&fixture.session)?;
    std::fs::remove_file(fixture.session.join("catalog.json"))?;

    // When
    let recovered = reload_recordings(&fixture)?;

    // Then
    assert_eq!(recovered.len(), expected.len());
    assert!(recovered.iter().all(|fragment| {
        fragment.recovery == RecordingRecovery::Recovered
            && fragment.gaps.iter().any(|gap| gap == "catalog_missing")
    }));
    assert_eq!(fragment_hashes(&fixture.session)?, expected);
    Ok(())
}

#[test]
fn startup_recovers_corrupt_catalog_and_later_fragments() -> anyhow::Result<()> {
    // Given
    let fixture = recorded_fixture("corrupt-catalog")?;
    let source = first_fragment(&fixture.session)?;
    let later = fixture.session.join("segment-99999.mkv");
    std::fs::copy(&source, &later)?;
    let expected = fragment_hashes(&fixture.session)?;
    std::fs::write(fixture.session.join("catalog.json"), b"not-json")?;

    // When
    let recovered = reload_recordings(&fixture)?;

    // Then
    assert_eq!(recovered.len(), expected.len());
    assert!(recovered.iter().all(|fragment| {
        fragment.recovery == RecordingRecovery::Recovered
            && fragment.gaps.iter().any(|gap| gap == "catalog_corrupt")
    }));
    assert_eq!(fragment_hashes(&fixture.session)?, expected);
    Ok(())
}

#[test]
fn startup_appends_decodable_fragment_missing_from_valid_catalog() -> anyhow::Result<()> {
    // Given
    let fixture = recorded_fixture("later-fragment")?;
    let source = first_fragment(&fixture.session)?;
    let later = fixture.session.join("segment-99999.mkv");
    std::fs::copy(&source, &later)?;
    let expected = fragment_hashes(&fixture.session)?;

    // When
    let recovered = reload_recordings(&fixture)?;

    // Then
    let appended = recovered
        .iter()
        .find(|fragment| fragment.path == later)
        .ok_or_else(|| anyhow::anyhow!("later fragment was hidden"))?;
    assert!(appended.decoded);
    assert_eq!(appended.recovery, RecordingRecovery::Recovered);
    assert!(
        appended
            .gaps
            .iter()
            .any(|gap| gap == "catalog_missing_fragment")
    );
    assert_eq!(fragment_hashes(&fixture.session)?, expected);
    Ok(())
}

#[test]
fn startup_keeps_malformed_tail_visible_without_rewriting_raw_files() -> anyhow::Result<()> {
    // Given
    let fixture = recorded_fixture("partial-tail")?;
    let tail = fixture.session.join("segment-99999.mkv");
    std::fs::write(&tail, b"incomplete-matroska-tail")?;
    let expected = fragment_hashes(&fixture.session)?;

    // When
    let recovered = reload_recordings(&fixture)?;

    // Then
    let partial = recovered
        .iter()
        .find(|fragment| fragment.path == tail)
        .ok_or_else(|| anyhow::anyhow!("malformed tail was hidden"))?;
    assert_eq!(partial.recovery, RecordingRecovery::Partial);
    assert!(!partial.decoded);
    assert!(
        partial
            .gaps
            .iter()
            .any(|gap| gap == "catalog_missing_fragment")
    );
    assert_eq!(fragment_hashes(&fixture.session)?, expected);
    Ok(())
}

fn first_fragment(session: &std::path::Path) -> anyhow::Result<std::path::PathBuf> {
    std::fs::read_dir(session)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|extension| extension == "mkv"))
        .ok_or_else(|| anyhow::anyhow!("missing fragment"))
}
