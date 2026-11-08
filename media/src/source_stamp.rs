use gstreamer as gst;

use crate::recording_provenance::RecordingTrackKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceStamp {
    pub engine_run_id: String,
    pub source_id: String,
    pub track_id: String,
    pub track_kind: RecordingTrackKind,
    pub continuity_generation: u64,
    pub media_pts_ns: u64,
    pub duration_ns: u64,
}

impl SourceStamp {
    pub fn from_buffer(buffer: &gst::BufferRef) -> Option<Self> {
        let meta = buffer
            .iter_meta::<gst::ReferenceTimestampMeta>()
            .filter(|meta| {
                meta.reference()
                    .structure(0)
                    .is_some_and(|value| value.name() == "timestamp/x-live-production-engine")
            })
            .last()?;
        let reference = meta.reference().structure(0)?;
        let track_kind = match reference.get::<&str>("track-kind").ok()? {
            "video" => RecordingTrackKind::Video,
            "audio" => RecordingTrackKind::Audio,
            _ => return None,
        };
        Some(Self {
            engine_run_id: reference.get::<&str>("engine-run-id").ok()?.to_owned(),
            source_id: reference.get::<&str>("source-id").ok()?.to_owned(),
            track_id: reference.get::<&str>("track-id").ok()?.to_owned(),
            track_kind,
            continuity_generation: reference.get("continuity-generation").ok()?,
            media_pts_ns: meta.timestamp().nseconds(),
            duration_ns: meta.duration().map_or_else(
                || buffer.duration().map_or(0, gst::ClockTime::nseconds),
                gst::ClockTime::nseconds,
            ),
        })
    }
}
