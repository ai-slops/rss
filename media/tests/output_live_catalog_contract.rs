mod output_support;

use gstreamer::{self as gst, prelude::*};
use live_production_media::output::{
    MediaTap, OutputDefinition, OutputId, OutputManager, RecordingDefinition,
};
use output_support::{producer, wait_for_media};
use std::time::{Duration, Instant};

#[test]
fn active_recording_publishes_session_and_closed_fragment_metadata() -> anyhow::Result<()> {
    gst::init()?;
    let root = tempfile::tempdir()?;
    let tap = MediaTap::new("durable-video", "durable-audio")?;
    let producer = producer(&tap)?;
    producer.set_state(gst::State::Playing)?;
    let manager = OutputManager::new();
    let output_id = OutputId::new("durable-session")?;
    manager.register(OutputDefinition::Recording(RecordingDefinition::new(
        output_id.clone(),
        tap,
        root.path(),
        Duration::from_secs(1),
    )?))?;
    manager.start(&output_id)?;
    wait_for_media(&manager, &output_id)?;
    let session = std::fs::read_dir(root.path())?
        .next()
        .transpose()?
        .ok_or_else(|| anyhow::anyhow!("missing active session"))?
        .path();
    let deadline = Instant::now() + Duration::from_secs(10);
    let (raw_count, catalog) = loop {
        let raw_count = std::fs::read_dir(&session)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "mkv"))
            .count();
        let catalog = std::fs::read(session.join("catalog.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
        if raw_count >= 3
            && catalog
                .as_ref()
                .and_then(serde_json::Value::as_array)
                .is_some_and(|entries| entries.len() >= 2)
        {
            break (
                raw_count,
                catalog.ok_or_else(|| anyhow::anyhow!("catalog"))?,
            );
        }
        if Instant::now() >= deadline {
            anyhow::bail!("closed fragments were not published before timeout");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(session.join("session.json"))?)?;
    let entries = catalog
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("catalog array"))?;
    assert_eq!(metadata["state"], "active");
    assert!(raw_count >= 3, "raw fragments: {raw_count}");
    assert!(entries.len() >= 2, "closed entries: {}", entries.len());
    assert!(
        entries.len() < raw_count,
        "active tail was published as closed"
    );
    assert!(entries.iter().all(|entry| entry["decoded"] == true));
    manager.stop(&output_id)?;
    let completed: serde_json::Value =
        serde_json::from_slice(&std::fs::read(session.join("session.json"))?)?;
    assert_eq!(completed["state"], "complete");
    assert!(std::fs::read_dir(&session)?.all(|entry| {
        entry.ok().is_none_or(|value| {
            !std::path::Path::new(&value.file_name())
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("tmp"))
        })
    }));
    producer.set_state(gst::State::Null)?;
    Ok(())
}
