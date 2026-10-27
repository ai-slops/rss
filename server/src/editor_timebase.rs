use serde::Serialize;

use crate::{
    editor::{EditorClockAnchor, EditorClockMapping, EditorJournal},
    model::Recording,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EditorTimebase {
    media_clock_domain: &'static str,
    wall_clock_domain: &'static str,
    mapping: Option<ClockMapping>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClockMapping {
    engine_run_id: String,
    media_pts_ns: String,
    wall_time_ns: String,
    bound_ns: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileTimeMapping {
    engine_run_id: String,
    media_start_ns: String,
    media_end_ns: String,
    wall_start_ns: String,
    wall_end_ns: String,
    uncertainty_ns: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum EditorIssue {
    MissingProgramReference,
    MissingClockMapping,
    UnknownRecordingTimebase,
    UnknownSourceGeneration,
}

pub(crate) const fn unresolved_timebase() -> EditorTimebase {
    EditorTimebase {
        media_clock_domain: "engineRunPtsNs",
        wall_clock_domain: "unixEpochNs",
        mapping: None,
    }
}

pub(crate) fn resolved_timebase(
    engine_run_id: &str,
    mapping: &EditorClockMapping,
) -> EditorTimebase {
    EditorTimebase {
        media_clock_domain: "engineRunPtsNs",
        wall_clock_domain: "unixEpochNs",
        mapping: Some(ClockMapping {
            engine_run_id: engine_run_id.to_owned(),
            media_pts_ns: mapping.media_time.clone(),
            wall_time_ns: mapping.wall_time.clone(),
            bound_ns: mapping.uncertainty.clone(),
        }),
    }
}

pub(crate) fn timebase(journal: &EditorJournal) -> EditorTimebase {
    clock_anchor(journal).map_or_else(unresolved_timebase, |anchor| {
        resolved_timebase(&anchor.engine_run_id, &anchor.mapping)
    })
}

pub(crate) fn has_clock_mapping(journal: &EditorJournal) -> bool {
    clock_anchor(journal).is_some()
}

pub(crate) fn file_time_mapping(
    recording: &Recording,
    journal: &EditorJournal,
) -> Option<FileTimeMapping> {
    let anchor = clock_anchor(journal)?;
    if recording.engine_run_id.as_deref()? != anchor.engine_run_id {
        return None;
    }
    let media_start_ns = recording.media_start_ns.as_deref()?.parse::<u64>().ok()?;
    let media_end_ns = recording.media_end_ns.as_deref()?.parse::<u64>().ok()?;
    let media_uncertainty_ns = recording
        .media_uncertainty_ns
        .as_deref()?
        .parse::<u64>()
        .ok()?;
    Some(FileTimeMapping {
        engine_run_id: anchor.engine_run_id.clone(),
        media_start_ns: media_start_ns.to_string(),
        media_end_ns: media_end_ns.to_string(),
        wall_start_ns: media_to_wall(&anchor.mapping, media_start_ns)?.to_string(),
        wall_end_ns: media_to_wall(&anchor.mapping, media_end_ns)?.to_string(),
        uncertainty_ns: media_uncertainty_ns
            .saturating_add(anchor.mapping.uncertainty.parse::<u64>().ok()?)
            .to_string(),
    })
}

fn clock_anchor(journal: &EditorJournal) -> Option<&EditorClockAnchor> {
    journal
        .clock_anchors
        .iter()
        .rev()
        .find(|anchor| anchor.engine_run_id == journal.engine_run_id.as_deref().unwrap_or_default())
}

fn media_to_wall(anchor: &EditorClockMapping, media_time_ns: u64) -> Option<u64> {
    let anchor_media = anchor.media_time.parse::<u64>().ok()?;
    let anchor_wall = anchor.wall_time.parse::<u64>().ok()?;
    if media_time_ns >= anchor_media {
        Some(anchor_wall.saturating_add(media_time_ns - anchor_media))
    } else {
        Some(anchor_wall.saturating_sub(anchor_media - media_time_ns))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        editor::EditorClockAnchor,
        model::{Id, Recording},
    };

    #[test]
    fn file_mapping_rejects_mismatched_engine_run() -> anyhow::Result<()> {
        let mut journal = serde_json::from_value::<EditorJournal>(serde_json::json!({
            "sessionId": "session",
            "active": false,
            "projectId": null,
            "engineRunId": "run-a",
            "startedAtNs": null,
            "observations": [],
            "markers": [],
            "gaps": [],
            "clockAnchors": []
        }))?;
        journal.clock_anchors.push(EditorClockAnchor {
            engine_run_id: "run-a".into(),
            mapping: EditorClockMapping {
                media_time: "10".into(),
                wall_time: "100".into(),
                uncertainty: "1".into(),
            },
        });
        let recording = Recording {
            id: Id("recording".into()),
            output_id: Id("output".into()),
            name: "legacy".into(),
            path: "legacy.mkv".into(),
            tap_point: "program".into(),
            session_id: "legacy-session".into(),
            recovery_status: "complete".into(),
            mapping_status: "mapped".into(),
            start_ns: "100".into(),
            end_ns: Some("200".into()),
            engine_run_id: Some("run-b".into()),
            media_start_ns: Some("10".into()),
            media_end_ns: Some("20".into()),
            media_uncertainty_ns: Some("1".into()),
            bytes: "1".into(),
            validation: "decoded".into(),
            sha256: None,
            gaps: Vec::new(),
            provenance: Vec::new(),
            provenance_gaps: Vec::new(),
        };
        assert!(file_time_mapping(&recording, &journal).is_none());
        Ok(())
    }
}
