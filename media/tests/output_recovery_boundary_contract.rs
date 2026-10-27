mod output_recovery_support;
mod output_support;

use std::time::Duration;

use live_production_media::output::{
    MediaTap, OutputDefinition, OutputId, OutputManager, RecordingDefinition, RecordingRecovery,
};
use output_recovery_support::{fragment_hashes, recorded_fixture, reload_recordings};

#[cfg(unix)]
#[test]
fn unreadable_recording_root_is_not_reported_as_empty() -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    // Given
    let directory = tempfile::tempdir()?;
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o000))?;
    let manager = OutputManager::new();
    let output_id = OutputId::new("unreadable-root")?;

    // When
    let result = manager.register(OutputDefinition::Recording(RecordingDefinition::new(
        output_id,
        MediaTap::new("unreadable-video", "unreadable-audio")?,
        directory.path(),
        Duration::from_secs(1),
    )?));

    // Then
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))?;
    assert!(result.is_err());
    Ok(())
}

#[test]
fn unreadable_catalog_recovers_raw_fragments_with_explicit_gap() -> anyhow::Result<()> {
    // Given
    let fixture = recorded_fixture("unreadable-catalog")?;
    std::fs::remove_file(fixture.session.join("catalog.json"))?;
    std::fs::create_dir(fixture.session.join("catalog.json"))?;

    // When
    let recovered = reload_recordings(&fixture)?;

    // Then
    assert!(!recovered.is_empty());
    assert!(recovered.iter().all(|fragment| {
        fragment.gaps.iter().any(|gap| gap == "catalog_unreadable")
            && fragment
                .gaps
                .iter()
                .any(|gap| gap == "catalog_repair_failed")
    }));
    Ok(())
}

#[test]
fn fresh_partial_inspection_cannot_be_promoted_by_stale_catalog() -> anyhow::Result<()> {
    // Given
    let fixture = recorded_fixture("stale-partial")?;
    let tail = fixture.session.join("segment-99999.mkv");
    std::fs::write(&tail, b"incomplete-tail")?;
    let tail_hash = fragment_hashes(&fixture.session)?
        .get("segment-99999.mkv")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("tail hash"))?;
    let catalog_path = fixture.session.join("catalog.json");
    let mut catalog: serde_json::Value = serde_json::from_slice(&std::fs::read(&catalog_path)?)?;
    let mut stale = catalog[0].clone();
    stale["path"] = serde_json::json!(tail);
    stale["sha256"] = serde_json::json!(tail_hash);
    stale["bytes"] = serde_json::json!(15);
    stale["decoded"] = serde_json::json!(true);
    stale["recovery"] = serde_json::json!("complete");
    stale["mapping"] = serde_json::json!("mapped");
    stale["media_start_ns"] = serde_json::json!(100);
    stale["media_end_ns"] = serde_json::json!(200);
    catalog
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("catalog array"))?
        .push(stale);
    std::fs::write(&catalog_path, serde_json::to_vec_pretty(&catalog)?)?;

    // When
    let recovered = reload_recordings(&fixture)?;

    // Then
    let partial = recovered
        .iter()
        .find(|fragment| fragment.path == tail)
        .ok_or_else(|| anyhow::anyhow!("tail missing"))?;
    assert!(!partial.decoded);
    assert_eq!(partial.recovery, RecordingRecovery::Partial);
    Ok(())
}

#[test]
fn interrupted_session_reclassifies_prior_complete_fragments() -> anyhow::Result<()> {
    // Given
    let fixture = recorded_fixture("interrupted-complete")?;
    let metadata_path = fixture.session.join("session.json");
    let mut metadata: serde_json::Value = serde_json::from_slice(&std::fs::read(&metadata_path)?)?;
    metadata["state"] = serde_json::json!("active");
    metadata["wall_end_ns"] = serde_json::Value::Null;
    std::fs::write(&metadata_path, serde_json::to_vec_pretty(&metadata)?)?;

    // When
    let recovered = reload_recordings(&fixture)?;

    // Then
    assert!(!recovered.is_empty());
    assert!(recovered.iter().all(|fragment| {
        fragment.recovery != RecordingRecovery::Complete
            && fragment.gaps.iter().any(|gap| gap == "abrupt_shutdown")
    }));
    Ok(())
}
