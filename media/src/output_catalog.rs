use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    output::{OutputError, RecordingRecovery, RecordingSegment},
    output_fragment::{self, FragmentContext},
    output_session::{self, RecordingSession, SessionMetadata},
    recording_clock::RecordingClock,
};

const CATALOG_FILE: &str = "catalog.json";

pub struct CatalogRead {
    pub entries: Vec<RecordingSegment>,
    pub issue: Option<&'static str>,
}

pub fn finalize(
    session: &RecordingSession,
    clock: Option<&RecordingClock>,
) -> Result<Vec<RecordingSegment>, OutputError> {
    let segments = inspect_all(
        &session.directory,
        &session.metadata,
        clock,
        RecordingRecovery::Complete,
        &session.metadata.gaps,
    )?;
    write(&session.directory, &segments)?;
    Ok(segments)
}

pub fn recover(
    session: &RecordingSession,
    clock: Option<&RecordingClock>,
) -> Result<Vec<RecordingSegment>, OutputError> {
    let mut gaps = session.metadata.gaps.clone();
    push_unique(&mut gaps, "recording_failed");
    let segments = inspect_all(
        &session.directory,
        &session.metadata,
        clock,
        RecordingRecovery::Recovered,
        &gaps,
    )?;
    let _ = write(&session.directory, &segments);
    Ok(segments)
}

pub fn publish_fragment(
    session: &RecordingSession,
    path: &Path,
    clock: Option<&RecordingClock>,
) -> Result<Vec<RecordingSegment>, OutputError> {
    if path.parent() != Some(session.directory.as_path())
        || path.extension().is_none_or(|extension| extension != "mkv")
    {
        return Err(OutputError::RecordingFinalization);
    }
    let mut catalog = read(&session.directory)?;
    if catalog
        .entries
        .iter()
        .any(|entry| same_file(&entry.path, path))
    {
        return Ok(catalog.entries);
    }
    catalog.entries.push(output_fragment::inspect(
        path,
        context(&session.metadata, clock, RecordingRecovery::Complete),
        session.metadata.gaps.clone(),
    )?);
    catalog
        .entries
        .sort_by(|left, right| left.path.cmp(&right.path));
    write(&session.directory, &catalog.entries)?;
    Ok(catalog.entries)
}

pub fn load(root: &Path) -> Result<Vec<RecordingSegment>, OutputError> {
    crate::output_discovery::load(root)
}

pub fn read(directory: &Path) -> Result<CatalogRead, OutputError> {
    match fs::read(directory.join(CATALOG_FILE)) {
        Ok(bytes) => serde_json::from_slice::<Vec<RecordingSegment>>(&bytes).map_or_else(
            |_| {
                Ok(CatalogRead {
                    entries: Vec::new(),
                    issue: Some("catalog_corrupt"),
                })
            },
            |entries| {
                Ok(CatalogRead {
                    entries,
                    issue: None,
                })
            },
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(CatalogRead {
            entries: Vec::new(),
            issue: Some("catalog_missing"),
        }),
        Err(_) => Ok(CatalogRead {
            entries: Vec::new(),
            issue: Some("catalog_unreadable"),
        }),
    }
}

pub fn write(directory: &Path, segments: &[RecordingSegment]) -> Result<(), OutputError> {
    output_session::atomic_json(&directory.join(CATALOG_FILE), &segments)
}

pub fn raw_paths(directory: &Path) -> Result<Vec<PathBuf>, OutputError> {
    let entries = fs::read_dir(directory).map_err(|_| OutputError::RecordingFinalization)?;
    let mut paths = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|_| OutputError::RecordingFinalization)?
            .path();
        if path.extension().is_some_and(|extension| extension == "mkv") {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

pub fn context<'a>(
    metadata: &'a SessionMetadata,
    clock: Option<&'a RecordingClock>,
    recovery: RecordingRecovery,
) -> FragmentContext<'a> {
    FragmentContext {
        session_id: &metadata.session_id,
        wall_start_ns: metadata.wall_start_ns,
        wall_end_ns: metadata.wall_end_ns,
        engine_run_id: metadata.engine_run_id.as_deref(),
        clock,
        recovery,
        provenance: &metadata.provenance,
        provenance_gaps: &metadata.provenance_gaps,
    }
}

pub fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|candidate| candidate == value) {
        values.push(value.to_owned());
    }
}

pub fn extend_unique(values: &mut Vec<String>, additions: &[String]) {
    for addition in additions {
        push_unique(values, addition);
    }
}

fn inspect_all(
    directory: &Path,
    metadata: &SessionMetadata,
    clock: Option<&RecordingClock>,
    recovery: RecordingRecovery,
    gaps: &[String],
) -> Result<Vec<RecordingSegment>, OutputError> {
    raw_paths(directory)?
        .into_iter()
        .filter(|path| !empty_waiting_fragment(path, metadata))
        .map(|path| {
            output_fragment::inspect(&path, context(metadata, clock, recovery), gaps.to_vec())
        })
        .collect()
}

fn same_file(left: &Path, right: &Path) -> bool {
    left.file_name() == right.file_name()
}

// A cleanly stopped, never-fed appsrc pipeline can leave a zero-byte placeholder.
// Keep the file, but do not present it as damaged captured media.
pub fn empty_waiting_fragment(path: &Path, metadata: &SessionMetadata) -> bool {
    metadata.state == output_session::SessionState::Complete
        && metadata.provenance.is_empty()
        && fs::metadata(path).is_ok_and(|file| file.len() == 0)
}

#[cfg(test)]
mod waiting_tests {
    use super::*;

    #[test]
    fn empty_complete_wait_is_not_media_but_failed_empty_capture_remains_visible()
    -> anyhow::Result<()> {
        let root = tempfile::tempdir()?;
        let mut session = output_session::RecordingSession::begin(output_session::SessionStart {
            root: root.path(),
            engine_run_id: None,
            clock_anchor: None,
            wall_start_ns: 1,
        })?;
        let path = session.directory.join("segment-00000.mkv");
        fs::write(&path, [])?;
        session.finish(output_session::SessionState::Complete, 2)?;
        assert!(finalize(&session, None)?.is_empty());
        assert!(load(root.path())?.is_empty());
        assert!(path.is_file());
        session.finish(output_session::SessionState::Failed, 3)?;
        let failed = load(root.path())?;
        assert_eq!(failed.len(), 1);
        assert!(!failed[0].decoded);
        assert!(failed[0].gaps.iter().any(|gap| gap == "recording_failed"));
        Ok(())
    }
}
