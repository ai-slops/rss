use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use crate::protocol_output::{OutputObservedState, OutputSnapshot};
pub use crate::protocol_runtime::*;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IdentifierError {
    #[error("identifier must contain 1 to 64 ASCII letters, digits, '.', '-', or '_'")]
    Invalid,
}

fn validate_identifier(value: &str) -> Result<(), IdentifierError> {
    let valid_length = !value.is_empty() && value.len() <= 64;
    let valid_chars = value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'));
    if valid_length && valid_chars {
        Ok(())
    } else {
        Err(IdentifierError::Invalid)
    }
}

macro_rules! identifier {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: &str) -> Result<Self, IdentifierError> {
                validate_identifier(value)?;
                Ok(Self(value.to_owned()))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

identifier!(CommandId);
identifier!(EngineRunId);
identifier!(OutputId);
identifier!(PlacementId);
identifier!(ProgramRevision);
identifier!(SourceId);
identifier!(TrackId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Milliseconds(i64);

impl Milliseconds {
    #[must_use]
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nanoseconds(u64);

impl Nanoseconds {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sequence(u64);

impl Sequence {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    pub id: CommandId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_engine_run_id: Option<EngineRunId>,
    #[serde(flatten)]
    pub kind: CommandKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CommandKind {
    AddSource {
        source: SourceSpec,
    },
    ConfigureSource {
        source: SourceSpec,
    },
    ApplyTiming {
        program_holdback: Milliseconds,
        video_correction: Milliseconds,
        audio_correction: Milliseconds,
    },
    ApplyTrackTiming {
        program_holdback: Milliseconds,
        corrections: Vec<TrackCorrection>,
    },
    RemoveSource {
        source_id: SourceId,
    },
    ReconnectSource {
        source_id: SourceId,
    },
    PrepareComposition {
        revision: ProgramRevision,
        placements: Vec<CompositionPlacement>,
    },
    QueryRuntime,
    ApplyAudioMix {
        routes: Vec<AudioMixRoute>,
    },
    PreparePreview {
        revision: ProgramRevision,
        placements: Vec<ScenePlacement>,
    },
    QuerySnapshot,
    Shutdown,
    StartOutput {
        output_id: OutputId,
    },
    StopOutput {
        output_id: OutputId,
    },
    TakePrepared {
        revision: ProgramRevision,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_program_revision: Option<ProgramRevision>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioMixRoute {
    pub track_id: TrackId,
    pub gain_milli_db: i32,
    pub muted: bool,
    pub routed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackCorrection {
    pub track_id: TrackId,
    pub correction: Milliseconds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenePlacement {
    pub slot: u16,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub opacity_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineEvent {
    pub sequence: Sequence,
    pub engine_run_id: EngineRunId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_id: Option<CommandId>,
    #[serde(rename = "at_ns")]
    pub at: Nanoseconds,
    #[serde(flatten)]
    pub kind: EventKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventKind {
    Accepted,
    Applied {
        discontinuity: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        media_at: Option<Nanoseconds>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        program_revision: Option<ProgramRevision>,
    },
    Failed {
        reason: String,
    },
    Rejected {
        reason: String,
    },
    Snapshot {
        outputs: Vec<OutputSnapshot>,
    },
    RuntimeSnapshot {
        sources: Vec<SourceSnapshot>,
        program: ProgramProgress,
        clock: EngineClockMapping,
    },
}

macro_rules! decimal_u64_serde {
    ($name:ident) => {
        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(&self.0.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = <&str>::deserialize(deserializer)?;
                value.parse().map(Self).map_err(serde::de::Error::custom)
            }
        }
    };
}

decimal_u64_serde!(Nanoseconds);
decimal_u64_serde!(Sequence);
