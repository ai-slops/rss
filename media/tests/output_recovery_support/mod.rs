use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

use gstreamer::prelude::*;
use live_production_media::output::{
    MediaTap, OutputDefinition, OutputId, OutputManager, RecordingDefinition, RecordingSegment,
};
use sha2::Digest;

use crate::output_support::{producer, wait_for_media};

pub struct RecordedFixture {
    pub root: tempfile::TempDir,
    pub session: PathBuf,
    output_id: OutputId,
    tap: MediaTap,
}

pub fn recorded_fixture(name: &str) -> anyhow::Result<RecordedFixture> {
    gstreamer::init()?;
    let root = tempfile::tempdir()?;
    let tap = MediaTap::new(&format!("{name}-video"), &format!("{name}-audio"))?;
    let producer = producer(&tap)?;
    producer.set_state(gstreamer::State::Playing)?;
    let output_id = OutputId::new(name)?;
    let manager = OutputManager::new();
    manager.register(OutputDefinition::Recording(RecordingDefinition::new(
        output_id.clone(),
        tap.clone(),
        root.path(),
        Duration::from_secs(1),
    )?))?;
    manager.start(&output_id)?;
    wait_for_media(&manager, &output_id)?;
    std::thread::sleep(Duration::from_secs(2));
    manager.stop(&output_id)?;
    producer.set_state(gstreamer::State::Null)?;
    drop(manager);
    let session = std::fs::read_dir(root.path())?
        .next()
        .transpose()?
        .ok_or_else(|| anyhow::anyhow!("missing session"))?
        .path();
    Ok(RecordedFixture {
        root,
        session,
        output_id,
        tap,
    })
}

pub fn reload_recordings(fixture: &RecordedFixture) -> anyhow::Result<Vec<RecordingSegment>> {
    let manager = OutputManager::new();
    manager.register(OutputDefinition::Recording(RecordingDefinition::new(
        fixture.output_id.clone(),
        fixture.tap.clone(),
        fixture.root.path(),
        Duration::from_secs(1),
    )?))?;
    manager.recordings(&fixture.output_id).map_err(Into::into)
}

pub fn fragment_hashes(session: &Path) -> anyhow::Result<BTreeMap<String, String>> {
    let mut hashes = BTreeMap::new();
    for entry in std::fs::read_dir(session)? {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "mkv") {
            let bytes = std::fs::read(&path)?;
            hashes.insert(
                path.file_name()
                    .and_then(std::ffi::OsStr::to_str)
                    .ok_or_else(|| anyhow::anyhow!("fragment name"))?
                    .to_owned(),
                format!("{:x}", sha2::Sha256::digest(bytes)),
            );
        }
    }
    Ok(hashes)
}
