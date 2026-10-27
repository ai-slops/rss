use gst::prelude::*;
use gstreamer as gst;

use crate::{
    engine_owner::now_nanoseconds,
    protocol::{EngineClockMapping, FrameCount, Nanoseconds, ProgramProgress},
};

pub fn program_progress(pipeline: &gst::Pipeline) -> ProgramProgress {
    let (video_frames, last_video_at) = sink_progress(pipeline, "program_video_tap");
    let (audio_buffers, last_audio_at) = sink_progress(pipeline, "program_audio_tap");
    ProgramProgress {
        video_frames: FrameCount::new(video_frames),
        audio_buffers: FrameCount::new(audio_buffers),
        last_video_at,
        last_audio_at,
    }
}

pub fn clock_mapping(pipeline: &gst::Pipeline) -> EngineClockMapping {
    let before = now_nanoseconds().value();
    let media_time = pipeline
        .current_running_time()
        .map_or(0, gst::ClockTime::nseconds);
    let after = now_nanoseconds().value();
    EngineClockMapping {
        media_time: Nanoseconds::new(media_time),
        wall_time: Nanoseconds::new(before.saturating_add(after.saturating_sub(before) / 2)),
        uncertainty: Nanoseconds::new(after.saturating_sub(before) / 2),
    }
}

pub fn media_time(pipeline: &gst::Pipeline) -> Nanoseconds {
    Nanoseconds::new(
        pipeline
            .current_running_time()
            .map_or(0, gst::ClockTime::nseconds),
    )
}

fn sink_progress(pipeline: &gst::Pipeline, name: &str) -> (u64, Option<Nanoseconds>) {
    let Some(sink) = pipeline.by_name(name) else {
        return (0, None);
    };
    let stats = sink.property::<gst::Structure>("stats");
    let rendered = stats.get::<u64>("rendered").unwrap_or(0);
    let last_at = sink
        .property::<Option<gst::Sample>>("last-sample")
        .and_then(|sample| sample.buffer_owned())
        .and_then(|buffer| buffer.pts())
        .map(|pts| Nanoseconds::new(pts.nseconds()));
    (rendered, last_at)
}
