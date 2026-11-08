use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::source_stamp::SourceStamp;
use gst::prelude::*;
use gstreamer as gst;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingTrackKind {
    Video,
    Audio,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordingProvenanceInterval {
    pub engine_run_id: String,
    pub source_id: String,
    pub track_id: String,
    pub track_kind: RecordingTrackKind,
    pub continuity_generation: u64,
    #[serde(with = "crate::decimal_u64")]
    pub local_start_ns: u64,
    #[serde(with = "crate::decimal_u64")]
    pub local_end_ns: u64,
    #[serde(with = "crate::decimal_u64")]
    pub media_start_ns: u64,
    #[serde(with = "crate::decimal_u64")]
    pub media_end_ns: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordingProvenanceGap {
    pub source_id: String,
    pub track_id: String,
    pub from_generation: u64,
    pub to_generation: u64,
    #[serde(with = "crate::decimal_u64")]
    pub local_start_ns: u64,
    #[serde(with = "crate::decimal_u64")]
    pub local_end_ns: u64,
    #[serde(with = "crate::decimal_u64")]
    pub media_start_ns: u64,
    #[serde(with = "crate::decimal_u64")]
    pub media_end_ns: u64,
}

#[derive(Default)]
struct ProvenanceState {
    intervals: Vec<RecordingProvenanceInterval>,
    gaps: Vec<RecordingProvenanceGap>,
    latest_by_track: HashMap<String, usize>,
}

#[derive(Default)]
pub struct ProvenanceCollector {
    state: Mutex<ProvenanceState>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProvenanceSnapshot {
    pub intervals: Vec<RecordingProvenanceInterval>,
    pub gaps: Vec<RecordingProvenanceGap>,
}

impl ProvenanceCollector {
    pub fn observe(self: &Arc<Self>, pad: &gst::Pad) -> Option<gst::PadProbeId> {
        let collector = Arc::clone(self);
        pad.add_probe(gst::PadProbeType::BUFFER, move |_, info| {
            if let Some(buffer) = info.buffer() {
                observe_buffer(&collector, buffer);
            }
            gst::PadProbeReturn::Ok
        })
    }

    pub fn snapshot(&self) -> Option<ProvenanceSnapshot> {
        let state = self.state.lock().ok()?;
        Some(ProvenanceSnapshot {
            intervals: state.intervals.clone(),
            gaps: state.gaps.clone(),
        })
    }

    pub fn record(&self, stamp: &SourceStamp, local_start_ns: u64) {
        append_interval(
            self,
            RecordingProvenanceInterval {
                engine_run_id: stamp.engine_run_id.clone(),
                source_id: stamp.source_id.clone(),
                track_id: stamp.track_id.clone(),
                track_kind: stamp.track_kind,
                continuity_generation: stamp.continuity_generation,
                local_start_ns,
                local_end_ns: local_start_ns.saturating_add(stamp.duration_ns),
                media_start_ns: stamp.media_pts_ns,
                media_end_ns: stamp.media_pts_ns.saturating_add(stamp.duration_ns),
            },
        );
    }
}

fn observe_buffer(collector: &ProvenanceCollector, buffer: &gst::BufferRef) {
    let Some(local_start) = buffer.pts().map(gst::ClockTime::nseconds) else {
        return;
    };
    let duration = buffer.duration().map_or(0, gst::ClockTime::nseconds);
    let local_end = local_start.saturating_add(duration);
    for meta in buffer.iter_meta::<gst::ReferenceTimestampMeta>() {
        let Some(reference) = meta.reference().structure(0) else {
            continue;
        };
        if reference.name() != "timestamp/x-live-production-engine" {
            continue;
        }
        let Ok(engine_run_id) = reference.get::<&str>("engine-run-id") else {
            continue;
        };
        let Ok(source_id) = reference.get::<&str>("source-id") else {
            continue;
        };
        let Ok(track_id) = reference.get::<&str>("track-id") else {
            continue;
        };
        let Ok(track_kind) = reference.get::<&str>("track-kind") else {
            continue;
        };
        let Ok(generation) = reference.get::<u64>("continuity-generation") else {
            continue;
        };
        let kind = match track_kind {
            "video" => RecordingTrackKind::Video,
            "audio" => RecordingTrackKind::Audio,
            _ => continue,
        };
        let media_start = meta.timestamp().nseconds();
        let media_end =
            media_start.saturating_add(meta.duration().map_or(duration, gst::ClockTime::nseconds));
        append_interval(
            collector,
            RecordingProvenanceInterval {
                engine_run_id: engine_run_id.to_owned(),
                source_id: source_id.to_owned(),
                track_id: track_id.to_owned(),
                track_kind: kind,
                continuity_generation: generation,
                local_start_ns: local_start,
                local_end_ns: local_end,
                media_start_ns: media_start,
                media_end_ns: media_end,
            },
        );
    }
}

fn append_interval(collector: &ProvenanceCollector, value: RecordingProvenanceInterval) {
    let Ok(mut state) = collector.state.lock() else {
        return;
    };
    if let Some(index) = state.latest_by_track.get(&value.track_id).copied() {
        let previous = &mut state.intervals[index];
        if previous.continuity_generation == value.continuity_generation
            && previous.engine_run_id == value.engine_run_id
        {
            previous.local_end_ns = previous.local_end_ns.max(value.local_end_ns);
            previous.media_end_ns = previous.media_end_ns.max(value.media_end_ns);
            return;
        }
        let from_generation = previous.continuity_generation;
        let local_start_ns = previous.local_end_ns;
        let media_start_ns = previous.media_end_ns;
        state.gaps.push(RecordingProvenanceGap {
            source_id: value.source_id.clone(),
            track_id: value.track_id.clone(),
            from_generation,
            to_generation: value.continuity_generation,
            local_start_ns,
            local_end_ns: value.local_start_ns.max(local_start_ns),
            media_start_ns,
            media_end_ns: value.media_start_ns.max(media_start_ns),
        });
    }
    let index = state.intervals.len();
    state.latest_by_track.insert(value.track_id.clone(), index);
    state.intervals.push(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interval(value: u64) -> RecordingProvenanceInterval {
        RecordingProvenanceInterval {
            engine_run_id: "run".into(),
            source_id: "source".into(),
            track_id: "track".into(),
            track_kind: RecordingTrackKind::Video,
            continuity_generation: 1,
            local_start_ns: value,
            local_end_ns: value + 1,
            media_start_ns: value + 2,
            media_end_ns: value + 3,
        }
    }

    #[test]
    fn nanoseconds_roundtrip_as_exact_decimal_strings() -> anyhow::Result<()> {
        let value = 9_007_199_254_740_993_u64;
        let encoded = serde_json::to_value(interval(value))?;
        assert_eq!(encoded["local_start_ns"], value.to_string());
        assert_eq!(encoded["media_end_ns"], (value + 3).to_string());
        let decoded = serde_json::from_value::<RecordingProvenanceInterval>(encoded)?;
        assert_eq!(decoded.local_start_ns, value);
        assert_eq!(decoded.media_end_ns, value + 3);
        Ok(())
    }

    #[test]
    fn legacy_numeric_nanoseconds_remain_readable() -> anyhow::Result<()> {
        let decoded = serde_json::from_value::<RecordingProvenanceInterval>(serde_json::json!({
            "engine_run_id":"run","source_id":"source","track_id":"track","track_kind":"video",
            "continuity_generation":1,"local_start_ns":10,"local_end_ns":20,
            "media_start_ns":30,"media_end_ns":40
        }))?;
        assert_eq!((decoded.local_start_ns, decoded.media_end_ns), (10, 40));
        Ok(())
    }
}
