use serde::{Deserialize, Serialize};

use crate::protocol::{Milliseconds, Nanoseconds, PlacementId, SourceId, TrackId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpec {
    pub id: SourceId,
    pub video_track_id: TrackId,
    pub audio_track_id: TrackId,
    #[serde(flatten)]
    pub kind: SourceKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source_kind", rename_all = "snake_case")]
pub enum SourceKind {
    Synthetic {
        pattern: SyntheticPattern,
        frequency_hz: u32,
    },
    SrtListener {
        bind_address: String,
        port: u16,
        latency: Milliseconds,
        passphrase: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyntheticPattern {
    Ball,
    Black,
    Smpte,
    Snow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositionPlacement {
    pub id: PlacementId,
    pub content: PlacementContent,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub crop: NormalizedCrop,
    pub fit: FitMode,
    pub opacity_milli: u16,
    pub z_order: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "content_kind", rename_all = "snake_case")]
pub enum PlacementContent {
    Track {
        track_id: TrackId,
    },
    Solid {
        argb: u32,
    },
    Text {
        text: String,
        argb: u32,
        font_size: u32,
    },
    Image {
        path: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedCrop {
    pub left_milli: u16,
    pub top_milli: u16,
    pub right_milli: u16,
    pub bottom_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitMode {
    Contain,
    Cover,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContinuityGeneration(u64);

impl ContinuityGeneration {
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
pub struct FrameCount(u64);

impl FrameCount {
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
pub struct SourceSnapshot {
    pub id: SourceId,
    pub continuity_generation: ContinuityGeneration,
    pub connected: bool,
    pub video_received_frames: FrameCount,
    pub audio_received_buffers: FrameCount,
    pub video_received_pts: Option<Nanoseconds>,
    pub video_playout_pts: Option<Nanoseconds>,
    pub video_delay: Milliseconds,
    pub audio_delay: Milliseconds,
    pub video_frames: FrameCount,
    pub audio_buffers: FrameCount,
    pub audio_input_peak_milli_db: Option<i32>,
    pub audio_post_peak_milli_db: Option<i32>,
    pub audio_clipping: Option<bool>,
    pub video_buffered_ns: Nanoseconds,
    pub audio_buffered_ns: Nanoseconds,
    pub gaps: Vec<SourceGap>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceGap {
    pub timebase: GapTimebase,
    pub generation: ContinuityGeneration,
    pub started_at: Nanoseconds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<Nanoseconds>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapTimebase {
    EngineMedia,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramProgress {
    pub video_frames: FrameCount,
    pub audio_buffers: FrameCount,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_video_at: Option<Nanoseconds>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_audio_at: Option<Nanoseconds>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineClockMapping {
    pub media_time: Nanoseconds,
    pub wall_time: Nanoseconds,
    pub uncertainty: Nanoseconds,
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

decimal_u64_serde!(ContinuityGeneration);
decimal_u64_serde!(FrameCount);
