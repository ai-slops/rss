use serde::Serialize;

use crate::{
    editor::EditorJournal,
    editor_timebase::{FileTimeMapping, file_time_mapping},
    model::{Id, OutputAdapter, Recording},
    snapshot::Snapshot,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EditorMedia {
    recording: Recording,
    tap_point: String,
    source_id: Option<Id>,
    source_continuity_generation: Option<String>,
    interval_clock_domain: &'static str,
    file_time_mapping: Option<FileTimeMapping>,
}

pub(crate) fn media_inventory(
    snapshot: &Snapshot,
    journal: &EditorJournal,
) -> (Vec<EditorMedia>, Option<Recording>, bool) {
    let mut inventory = Vec::new();
    let mut program_reference = None;
    let mut all_mapped = true;
    for recording in &snapshot.recordings {
        let adapter = snapshot
            .project
            .outputs
            .iter()
            .find(|output| output.id == recording.output_id)
            .map(|output| &output.adapter);
        let configured_source_id = match adapter {
            Some(OutputAdapter::ProgramRecording { .. }) => {
                program_reference = Some(recording.clone());
                None
            }
            Some(OutputAdapter::SourceRecording { source_id, .. }) => Some(source_id.clone()),
            Some(OutputAdapter::Rtmps { .. }) | None => None,
        };
        let source_id = provenance_source_id(recording).or(configured_source_id);
        let source_continuity_generation = provenance_generation(recording);
        if recording.tap_point == "program" {
            program_reference = Some(recording.clone());
        }
        let file_time_mapping = file_time_mapping(recording, journal);
        all_mapped &= file_time_mapping.is_some();
        inventory.push(EditorMedia {
            recording: recording.clone(),
            tap_point: recording.tap_point.clone(),
            source_id,
            source_continuity_generation,
            interval_clock_domain: if file_time_mapping.is_some() {
                "engineRunPtsNs"
            } else {
                "unknown"
            },
            file_time_mapping,
        });
    }
    (inventory, program_reference, all_mapped)
}

fn provenance_source_id(recording: &Recording) -> Option<Id> {
    let first = recording.provenance.first()?.source_id.as_str();
    recording
        .provenance
        .iter()
        .all(|interval| interval.source_id == first)
        .then(|| Id(first.to_owned()))
}

fn provenance_generation(recording: &Recording) -> Option<String> {
    let first = recording.provenance.first()?.continuity_generation;
    recording
        .provenance
        .iter()
        .all(|interval| interval.continuity_generation == first)
        .then(|| first.to_string())
}
