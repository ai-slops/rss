use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::protocol::EngineRunId;

pub use crate::output_manager::OutputManager;
pub use crate::recording_provenance::{
    RecordingProvenanceGap, RecordingProvenanceInterval, RecordingTrackKind,
};

pub const PROGRAM_VIDEO_CHANNEL: &str = "live-program-video-v1";
pub const PROGRAM_AUDIO_CHANNEL: &str = "live-program-audio-v1";

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OutputId(String);

impl OutputId {
    pub fn new(value: &str) -> Result<Self, OutputError> {
        let valid = !value.is_empty()
            && value.len() <= 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'));
        valid
            .then(|| Self(value.to_owned()))
            .ok_or(OutputError::InvalidIdentifier)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaTap {
    video_channel: String,
    audio_channel: String,
}

impl MediaTap {
    pub fn new(video_channel: &str, audio_channel: &str) -> Result<Self, OutputError> {
        validate_channel(video_channel)?;
        validate_channel(audio_channel)?;
        Ok(Self {
            video_channel: video_channel.to_owned(),
            audio_channel: audio_channel.to_owned(),
        })
    }

    #[must_use]
    pub fn video_channel(&self) -> &str {
        &self.video_channel
    }

    #[must_use]
    pub fn audio_channel(&self) -> &str {
        &self.audio_channel
    }
}

#[derive(Clone)]
pub struct RecordingDefinition {
    pub(crate) id: OutputId,
    pub(crate) tap: MediaTap,
    pub(crate) directory: PathBuf,
    pub(crate) segment_duration: Duration,
    pub(crate) engine_run_id: Option<String>,
    pub(crate) capture_source_provenance: bool,
}

impl RecordingDefinition {
    pub fn new(
        id: OutputId,
        tap: MediaTap,
        directory: &Path,
        segment_duration: Duration,
    ) -> Result<Self, OutputError> {
        if segment_duration < Duration::from_secs(1) || segment_duration > Duration::from_secs(600)
        {
            return Err(OutputError::InvalidSegmentDuration);
        }
        Ok(Self {
            id,
            tap,
            directory: directory.to_path_buf(),
            segment_duration,
            engine_run_id: None,
            capture_source_provenance: false,
        })
    }

    #[must_use]
    pub fn with_engine_run_id(mut self, engine_run_id: &EngineRunId) -> Self {
        self.engine_run_id = Some(engine_run_id.as_str().to_owned());
        self
    }

    #[must_use]
    pub const fn with_source_provenance(mut self) -> Self {
        self.capture_source_provenance = true;
        self
    }
}

#[derive(Clone)]
pub enum OutputDefinition {
    Recording(RecordingDefinition),
    Rtmps {
        id: OutputId,
        tap: MediaTap,
        destination: DestinationUrl,
    },
}

impl OutputDefinition {
    #[must_use]
    pub const fn rtmps(id: OutputId, tap: MediaTap, destination: DestinationUrl) -> Self {
        Self::Rtmps {
            id,
            tap,
            destination,
        }
    }

    pub(crate) const fn id(&self) -> &OutputId {
        match self {
            Self::Recording(definition) => &definition.id,
            Self::Rtmps { id, .. } => id,
        }
    }
}

#[derive(Clone)]
pub struct DestinationUrl(String);

impl DestinationUrl {
    pub fn new(value: &str) -> Result<Self, OutputError> {
        let valid = value.starts_with("rtmps://")
            && value.len() <= 2_048
            && !value
                .bytes()
                .any(|byte| byte.is_ascii_whitespace() || byte == 0);
        valid
            .then(|| Self(value.to_owned()))
            .ok_or(OutputError::InvalidDestination)
    }

    pub(crate) fn expose_to_publisher(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputState {
    Failed,
    Finalizing,
    Retrying,
    Running,
    Starting,
    Stopped,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutputSnapshot {
    pub desired: OutputState,
    pub actual: OutputState,
    pub last_media_at_ns: Option<u64>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordingSegment {
    pub path: PathBuf,
    #[serde(default, with = "crate::decimal_u64::option")]
    pub start_ns: Option<u64>,
    #[serde(default, with = "crate::decimal_u64::option")]
    pub end_ns: Option<u64>,
    pub bytes: u64,
    pub sha256: String,
    pub decoded: bool,
    #[serde(default)]
    pub session_id: String,
    #[serde(default = "complete_recovery")]
    pub recovery: RecordingRecovery,
    #[serde(default)]
    pub mapping: RecordingMapping,
    #[serde(default)]
    pub gaps: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_run_id: Option<String>,
    #[serde(
        default,
        with = "crate::decimal_u64::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub media_start_ns: Option<u64>,
    #[serde(
        default,
        with = "crate::decimal_u64::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub media_end_ns: Option<u64>,
    #[serde(
        default,
        with = "crate::decimal_u64::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub media_uncertainty_ns: Option<u64>,
    #[serde(default)]
    pub provenance: Vec<RecordingProvenanceInterval>,
    #[serde(default)]
    pub provenance_gaps: Vec<RecordingProvenanceGap>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingRecovery {
    #[default]
    Complete,
    Recovered,
    Partial,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingMapping {
    Mapped,
    #[default]
    Unmapped,
}

const fn complete_recovery() -> RecordingRecovery {
    RecordingRecovery::Complete
}

#[derive(Debug, Error)]
pub enum OutputError {
    #[error("output is already registered")]
    AlreadyRegistered,
    #[error("destination must be an RTMPS URL")]
    InvalidDestination,
    #[error("output identifier is invalid")]
    InvalidIdentifier,
    #[error("recording segment duration must be between 1 and 600 seconds")]
    InvalidSegmentDuration,
    #[error("media tap channel is invalid")]
    InvalidTap,
    #[error("output owner stopped")]
    OwnerStopped,
    #[error("output pipeline failed")]
    Pipeline,
    #[error("recording finalization failed")]
    RecordingFinalization,
    #[error("output is not registered")]
    UnknownOutput,
}

fn validate_channel(value: &str) -> Result<(), OutputError> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'));
    valid.then_some(()).ok_or(OutputError::InvalidTap)
}
