use std::{collections::BTreeMap, fs, path::Path};

use crate::{
    output::{OutputError, RecordingMapping, RecordingRecovery, RecordingSegment},
    output_catalog, output_fragment,
    output_session::{self, SessionState},
};

pub fn load(root: &Path) -> Result<Vec<RecordingSegment>, OutputError> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(OutputError::RecordingFinalization),
    };
    let mut recordings = Vec::new();
    for entry in entries {
        let directory = entry
            .map_err(|_| OutputError::RecordingFinalization)?
            .path();
        if directory.is_dir() {
            recordings.extend(recover_directory(&directory)?);
        }
    }
    recordings.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(recordings)
}

fn recover_directory(directory: &Path) -> Result<Vec<RecordingSegment>, OutputError> {
    let mut session = output_session::read(directory);
    let catalog = output_catalog::read(directory)?;
    let mut issues = session.issues;
    if let Some(issue) = catalog.issue {
        output_catalog::push_unique(&mut issues, issue);
    }
    match session.metadata.state {
        SessionState::Active => output_catalog::push_unique(&mut issues, "abrupt_shutdown"),
        SessionState::Failed => output_catalog::push_unique(&mut issues, "recording_failed"),
        SessionState::Complete | SessionState::Recovered | SessionState::Unknown => {}
    }
    let existing = by_name(&catalog.entries);
    let paths = output_catalog::raw_paths(directory)?;
    let mut recordings = Vec::new();
    for path in &paths {
        let name = file_name(path)?;
        let mut gaps = session.metadata.gaps.clone();
        output_catalog::extend_unique(&mut gaps, &issues);
        if catalog.issue.is_none() && !existing.contains_key(&name) {
            output_catalog::push_unique(&mut gaps, "catalog_missing_fragment");
        }
        let mut inspected = output_fragment::inspect(
            path,
            output_catalog::context(&session.metadata, None, RecordingRecovery::Recovered),
            gaps,
        )?;
        if let Some(previous) = existing.get(&name)
            && previous.sha256 == inspected.sha256
        {
            preserve_catalog_metadata(&mut inspected, previous);
        }
        recordings.push(inspected);
    }
    for previous in &catalog.entries {
        if !paths.iter().any(|path| same_file(path, &previous.path)) {
            let mut missing = previous.clone();
            missing.decoded = false;
            missing.recovery = RecordingRecovery::Partial;
            output_catalog::push_unique(&mut missing.gaps, "raw_fragment_missing");
            recordings.push(missing);
        }
    }
    recordings.sort_by(|left, right| left.path.cmp(&right.path));
    if output_catalog::write(directory, &recordings).is_err() {
        for recording in &mut recordings {
            output_catalog::push_unique(&mut recording.gaps, "catalog_repair_failed");
        }
    }
    if session.metadata.state != SessionState::Complete {
        session.metadata.state = SessionState::Recovered;
        output_catalog::extend_unique(&mut session.metadata.gaps, &issues);
        if output_session::write_metadata(directory, &session.metadata).is_err() {
            for recording in &mut recordings {
                output_catalog::push_unique(&mut recording.gaps, "session_metadata_repair_failed");
            }
        }
    }
    Ok(recordings)
}

fn preserve_catalog_metadata(current: &mut RecordingSegment, previous: &RecordingSegment) {
    let interrupted = current
        .gaps
        .iter()
        .any(|gap| matches!(gap.as_str(), "abrupt_shutdown" | "recording_failed"));
    if current.decoded && !interrupted {
        current.recovery = previous.recovery;
    }
    if current.engine_run_id.is_none() {
        current.engine_run_id.clone_from(&previous.engine_run_id);
    }
    if current.media_start_ns.is_none() {
        current.media_start_ns = previous.media_start_ns;
    }
    if current.media_end_ns.is_none() {
        current.media_end_ns = previous.media_end_ns;
    }
    if current.media_uncertainty_ns.is_none() {
        current.media_uncertainty_ns = previous.media_uncertainty_ns;
    }
    if current.provenance.is_empty() {
        current.provenance.clone_from(&previous.provenance);
    }
    if current.provenance_gaps.is_empty() {
        current
            .provenance_gaps
            .clone_from(&previous.provenance_gaps);
    }
    current.mapping = if current.media_start_ns.is_some() && current.media_end_ns.is_some() {
        RecordingMapping::Mapped
    } else {
        RecordingMapping::Unmapped
    };
    output_catalog::extend_unique(&mut current.gaps, &previous.gaps);
}

fn by_name(segments: &[RecordingSegment]) -> BTreeMap<String, &RecordingSegment> {
    segments
        .iter()
        .filter_map(|segment| file_name(&segment.path).ok().map(|name| (name, segment)))
        .collect()
}

fn file_name(path: &Path) -> Result<String, OutputError> {
    path.file_name()
        .and_then(std::ffi::OsStr::to_str)
        .map(str::to_owned)
        .ok_or(OutputError::RecordingFinalization)
}

fn same_file(left: &Path, right: &Path) -> bool {
    left.file_name() == right.file_name()
}
