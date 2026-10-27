use serde::Serialize;

use crate::{
    editor::{EditorGap, EditorJournal, EditorObservation, TrackTiming},
    editor_timebase::{EditorIssue, EditorTimebase, unresolved_timebase},
    model::{Id, OutputAdapter, Recording, Scene},
    snapshot::{AudioRouteSnapshot, Snapshot},
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorPackage {
    schema_version: u32,
    session_id: String,
    project_id: Id,
    engine_run_id: String,
    media_inventory: Vec<EditorMedia>,
    program_reference: Option<Recording>,
    layout_use: Vec<LayoutUse>,
    audio_history: Vec<AudioHistory>,
    timing_history: Vec<TimingHistory>,
    markers: Vec<ExportMarker>,
    timebase: EditorTimebase,
    gaps: Vec<EditorGap>,
    issues: Vec<EditorIssue>,
    reconstruction: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EditorMedia {
    recording: Recording,
    tap_point: String,
    source_id: Option<Id>,
    interval_clock_domain: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LayoutUse {
    cursor: String,
    media_from_pts_ns: String,
    media_until_pts_ns: Option<String>,
    program_revision: String,
    scene: Scene,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AudioHistory {
    cursor: String,
    occurred_at_ns: String,
    clock_domain: &'static str,
    routes: Vec<AudioRouteSnapshot>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TimingHistory {
    cursor: String,
    occurred_at_ns: String,
    clock_domain: &'static str,
    holdback_ms: i64,
    tracks: Vec<TrackTiming>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportMarker {
    label: String,
    wall_time_ns: String,
}

pub(crate) fn build(journal: &EditorJournal, snapshot: &Snapshot) -> anyhow::Result<EditorPackage> {
    let session_id = journal
        .session_id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Start an editor session before export."))?;
    let project_id = journal
        .project_id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Editor session has no project identity."))?;
    let engine_run_id = journal
        .engine_run_id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Editor session has no engine identity."))?;
    let (media_inventory, program_reference) = media(snapshot);
    let mut issues = vec![EditorIssue::MissingClockMapping];
    if program_reference.is_none() {
        issues.push(EditorIssue::MissingProgramReference);
    }
    if !snapshot.recordings.is_empty() {
        issues.push(EditorIssue::UnknownRecordingTimebase);
    }
    Ok(EditorPackage {
        schema_version: 1,
        session_id,
        project_id,
        engine_run_id,
        media_inventory,
        program_reference,
        layout_use: layout_use(&journal.observations),
        audio_history: journal
            .observations
            .iter()
            .map(AudioHistory::from)
            .collect(),
        timing_history: journal
            .observations
            .iter()
            .map(TimingHistory::from)
            .collect(),
        markers: journal
            .markers
            .iter()
            .map(|marker| ExportMarker {
                label: marker.label.clone(),
                wall_time_ns: marker.at_ns.clone(),
            })
            .collect(),
        timebase: unresolved_timebase(),
        gaps: recording_gaps(journal, snapshot),
        issues,
        reconstruction: "approximate",
    })
}

fn media(snapshot: &Snapshot) -> (Vec<EditorMedia>, Option<Recording>) {
    let mut inventory = Vec::new();
    let mut program_reference = None;
    for recording in &snapshot.recordings {
        let adapter = snapshot
            .project
            .outputs
            .iter()
            .find(|output| output.id == recording.output_id)
            .map(|output| &output.adapter);
        let source_id = match adapter {
            Some(OutputAdapter::ProgramRecording { .. }) => {
                program_reference = Some(recording.clone());
                None
            }
            Some(OutputAdapter::SourceRecording { source_id, .. }) => Some(source_id.clone()),
            Some(OutputAdapter::Rtmps { .. }) | None => None,
        };
        if recording.tap_point == "program" {
            program_reference = Some(recording.clone());
        }
        inventory.push(EditorMedia {
            recording: recording.clone(),
            tap_point: recording.tap_point.clone(),
            source_id,
            interval_clock_domain: "unknown",
        });
    }
    (inventory, program_reference)
}

fn recording_gaps(journal: &EditorJournal, snapshot: &Snapshot) -> Vec<EditorGap> {
    let mut gaps = journal.gaps.clone();
    if journal
        .engine_run_id
        .as_ref()
        .is_some_and(|engine_run_id| engine_run_id != &snapshot.engine_run_id)
    {
        gaps.push(EditorGap {
            kind: "observation".into(),
            detail: "Media engine run changed during the editor session.".into(),
        });
    }
    gaps.extend(snapshot.recordings.iter().flat_map(|recording| {
        recording.gaps.iter().map(|detail| EditorGap {
            kind: "recording".into(),
            detail: detail.clone(),
        })
    }));
    gaps
}

fn layout_use(observations: &[EditorObservation]) -> Vec<LayoutUse> {
    let mut previous_revision: Option<&str> = None;
    let distinct = observations
        .iter()
        .filter(|observation| {
            if previous_revision == Some(observation.program_revision.as_str()) {
                false
            } else {
                previous_revision = Some(&observation.program_revision);
                true
            }
        })
        .collect::<Vec<_>>();
    distinct
        .iter()
        .enumerate()
        .map(|(index, observation)| LayoutUse {
            cursor: observation.cursor.clone(),
            media_from_pts_ns: observation.applied_at_ns.clone(),
            media_until_pts_ns: distinct
                .get(index.saturating_add(1))
                .map(|next| next.applied_at_ns.clone()),
            program_revision: observation.program_revision.clone(),
            scene: observation.scene.clone(),
        })
        .collect()
}

impl From<&EditorObservation> for AudioHistory {
    fn from(value: &EditorObservation) -> Self {
        Self {
            cursor: value.cursor.clone(),
            occurred_at_ns: value.occurred_at_ns.clone(),
            clock_domain: "commandApplicationUnspecified",
            routes: value.audio_routes.clone(),
        }
    }
}

impl From<&EditorObservation> for TimingHistory {
    fn from(value: &EditorObservation) -> Self {
        Self {
            cursor: value.cursor.clone(),
            occurred_at_ns: value.occurred_at_ns.clone(),
            clock_domain: "commandApplicationUnspecified",
            holdback_ms: value.holdback_ms,
            tracks: value.track_corrections.clone(),
        }
    }
}
