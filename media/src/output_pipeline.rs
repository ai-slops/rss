use std::time::Duration;

use gst::prelude::*;
use gstreamer as gst;

use crate::output::{MediaTap, OutputError};

pub fn recording_pipeline(
    tap: &MediaTap,
    pattern: &str,
    segment_duration: Duration,
) -> Result<gst::Pipeline, OutputError> {
    let segment_ns = segment_duration.as_nanos().try_into().unwrap_or(u64::MAX);
    parse(&format!(
        "splitmuxsink name=recording_mux location={} muxer-factory=matroskamux max-size-time={segment_ns} send-keyframe-requests=true async-finalize=true \
         intervideosrc name=recording_video_input channel={} timeout=1000000000 ! queue max-size-time=2000000000 ! videoconvert ! video/x-raw,format=I420 ! x264enc tune=zerolatency speed-preset=veryfast key-int-max=30 bitrate=2500 ! h264parse config-interval=-1 ! identity name=output_activity ! recording_mux.video \
         interaudiosrc name=recording_audio_input channel={} buffer-time=500000000 latency-time=25000000 ! queue max-size-time=2000000000 ! audioconvert ! audioresample ! audio/x-raw,rate=48000,channels=2 ! voaacenc bitrate=128000 ! aacparse ! recording_mux.audio_0",
        quote(pattern),
        tap.video_channel(),
        tap.audio_channel()
    ))
}

pub fn rtmps_pipeline(tap: &MediaTap, file_descriptor: i32) -> Result<gst::Pipeline, OutputError> {
    parse(&format!(
        "flvmux name=network_mux streamable=true enforce-increasing-timestamps=true ! identity name=output_activity ! fdsink fd={file_descriptor} async=false sync=false \
         intervideosrc channel={} timeout=1000000000 ! queue max-size-time=2000000000 ! videoconvert ! video/x-raw,format=I420 ! x264enc tune=zerolatency speed-preset=veryfast key-int-max=60 bitrate=2500 ! h264parse config-interval=-1 ! network_mux.video \
         interaudiosrc channel={} buffer-time=500000000 latency-time=25000000 ! queue max-size-time=2000000000 ! audioconvert ! audioresample ! audio/x-raw,rate=48000,channels=2 ! voaacenc bitrate=128000 ! aacparse ! network_mux.audio",
        tap.video_channel(),
        tap.audio_channel()
    ))
}

fn parse(description: &str) -> Result<gst::Pipeline, OutputError> {
    gst::init().map_err(|_| OutputError::Pipeline)?;
    gst::parse::launch(description)
        .map_err(|_| OutputError::Pipeline)?
        .downcast::<gst::Pipeline>()
        .map_err(|_| OutputError::Pipeline)
}

fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
