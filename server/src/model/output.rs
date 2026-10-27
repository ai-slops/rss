use super::Id;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub id: Id,
    pub name: String,
    pub adapter: OutputAdapter,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OutputAdapter {
    ProgramRecording {
        #[serde(rename = "segmentSeconds")]
        segment_seconds: u32,
    },
    SourceRecording {
        #[serde(rename = "sourceId")]
        source_id: Id,
        #[serde(rename = "segmentSeconds")]
        segment_seconds: u32,
    },
    Rtmps {
        #[serde(rename = "destinationId")]
        destination_id: Id,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DesiredState {
    Running,
    Stopped,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ActualState {
    Stopped,
    Starting,
    Running,
    Retrying,
    Finalizing,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputStatus {
    pub output_id: Id,
    pub desired: DesiredState,
    pub actual: ActualState,
    pub last_media_at_ns: Option<String>,
    pub error: Option<String>,
    pub platform_status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    pub id: Id,
    pub output_id: Id,
    pub name: String,
    pub path: String,
    pub tap_point: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default = "complete_recovery_status")]
    pub recovery_status: String,
    #[serde(default = "unmapped_status")]
    pub mapping_status: String,
    pub start_ns: String,
    pub end_ns: Option<String>,
    #[serde(default)]
    pub engine_run_id: Option<String>,
    #[serde(default)]
    pub media_start_ns: Option<String>,
    #[serde(default)]
    pub media_end_ns: Option<String>,
    #[serde(default)]
    pub media_uncertainty_ns: Option<String>,
    pub bytes: String,
    pub validation: String,
    pub sha256: Option<String>,
    pub gaps: Vec<String>,
    #[serde(default)]
    pub provenance: Vec<live_production_media::output::RecordingProvenanceInterval>,
    #[serde(default)]
    pub provenance_gaps: Vec<live_production_media::output::RecordingProvenanceGap>,
}

fn complete_recovery_status() -> String {
    "complete".to_owned()
}

fn unmapped_status() -> String {
    "unmapped".to_owned()
}
