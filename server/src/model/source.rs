use super::Id;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: Id,
    pub name: String,
    pub enabled: bool,
    pub adapter: SourceAdapter,
    pub tracks: Vec<Track>,
    pub record_on_connect: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SourceAdapter {
    Synthetic {
        pattern: String,
        #[serde(rename = "frequencyHz")]
        frequency_hz: u32,
    },
    Srt {
        #[serde(rename = "bindAddress", default = "default_srt_bind_address")]
        bind_address: String,
        #[serde(default = "default_srt_port")]
        port: u16,
        #[serde(rename = "bufferMs", default = "default_srt_buffer_ms")]
        buffer_ms: u32,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: Id,
    pub kind: TrackKind,
    pub correction_ms: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TrackKind {
    Video,
    Audio,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioRoute {
    pub track_id: Id,
    pub gain_db: f64,
    pub muted: bool,
    pub routed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackCorrection {
    pub track_id: Id,
    pub correction_ms: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimingChange {
    pub holdback_ms: i64,
    pub corrections: Vec<TrackCorrection>,
}

const fn default_srt_port() -> u16 {
    9_000
}
const fn default_srt_buffer_ms() -> u32 {
    120
}

fn default_srt_bind_address() -> String {
    "0.0.0.0".to_owned()
}
