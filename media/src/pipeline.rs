use std::path::{Path, PathBuf};

use crate::protocol::Milliseconds;
use crate::timing::{TimingLimits, TimingPlan, TimingRejection};

const MAXIMUM_BUFFER: Milliseconds = Milliseconds::new(5_000);
const VIDEO_QUEUE_BUFFERS: u16 = 8;
const AUDIO_QUEUE_BUFFERS: u16 = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramPipelineConfig {
    holdback: Milliseconds,
    video_correction: Milliseconds,
    audio_correction: Milliseconds,
    recording_pattern: PathBuf,
}

impl ProgramPipelineConfig {
    #[must_use]
    pub fn synthetic(
        holdback: Milliseconds,
        video_correction: Milliseconds,
        audio_correction: Milliseconds,
        recording_pattern: impl AsRef<Path>,
    ) -> Self {
        Self {
            holdback,
            video_correction,
            audio_correction,
            recording_pattern: recording_pattern.as_ref().to_path_buf(),
        }
    }
}

pub fn program_pipeline_description(
    config: &ProgramPipelineConfig,
) -> Result<String, TimingRejection> {
    let plan = TimingPlan::validate(
        &TimingLimits::new(MAXIMUM_BUFFER),
        config.holdback,
        config.video_correction,
        config.audio_correction,
    );
    let plan = plan?;
    let video_offset = milliseconds_to_nanoseconds(plan.video_delay());
    let audio_offset = milliseconds_to_nanoseconds(plan.audio_delay());
    let location = quote_property(config.recording_pattern.to_string_lossy().as_ref());

    Ok(format!(
        "compositor name=program_compositor sink_0::xpos=0 sink_0::width=640 sink_0::height=720 sink_1::xpos=640 sink_1::width=640 sink_1::height=720 ! video/x-raw,width=1280,height=720,framerate=30/1 ! tee name=program_video \
         compositor name=preview_compositor sink_0::xpos=0 sink_0::width=960 sink_0::height=540 sink_1::xpos=640 sink_1::ypos=360 sink_1::width=320 sink_1::height=180 ! video/x-raw,width=960,height=540,framerate=30/1 ! queue max-size-buffers=1 leaky=downstream ! identity name=stalled_preview sleep-time=500000 ! fakesink name=preview_video sync=false \
         audiomixer name=program_mixer ! audio/x-raw,rate=48000,channels=2 ! tee name=program_audio \
         splitmuxsink name=program_recorder location={location} muxer-factory=matroskamux max-size-time=2000000000 send-keyframe-requests=true \
         videotestsrc name=video_timing_source is-live=true pattern=smpte timestamp-offset={video_offset} ! video/x-raw,width=640,height=720,framerate=30/1 ! tee name=source_zero_video \
         source_zero_video. ! identity name=video_timing ! queue max-size-buffers={VIDEO_QUEUE_BUFFERS} leaky=downstream ! program_compositor.sink_0 \
         source_zero_video. ! queue max-size-buffers=1 leaky=downstream ! preview_compositor.sink_0 \
         videotestsrc is-live=true pattern=ball ! video/x-raw,width=640,height=720,framerate=30/1 ! tee name=source_one_video \
         source_one_video. ! queue max-size-buffers={VIDEO_QUEUE_BUFFERS} leaky=downstream ! program_compositor.sink_1 \
         source_one_video. ! queue max-size-buffers=1 leaky=downstream ! preview_compositor.sink_1 \
         audiotestsrc name=audio_timing_source is-live=true wave=sine freq=440 timestamp-offset={audio_offset} ! audioconvert ! audioresample ! identity name=audio_timing ! queue max-size-buffers={AUDIO_QUEUE_BUFFERS} leaky=downstream ! program_mixer.sink_0 \
         audiotestsrc is-live=true wave=sine freq=660 ! audioconvert ! audioresample ! queue max-size-buffers={AUDIO_QUEUE_BUFFERS} leaky=downstream ! program_mixer.sink_1 \
         program_video. ! queue max-size-buffers={VIDEO_QUEUE_BUFFERS} ! videoconvert ! x264enc tune=zerolatency key-int-max=30 ! h264parse ! program_recorder.video \
         program_audio. ! queue max-size-buffers={AUDIO_QUEUE_BUFFERS} ! audioconvert ! voaacenc ! aacparse ! program_recorder.audio_0 \
         program_audio. ! queue max-size-buffers=4 leaky=downstream ! fakesink name=preview_audio sync=false"
    ))
}

#[must_use]
pub fn timing_fixture_description(recording: &Path, correction: Milliseconds) -> String {
    let location = quote_property(recording.to_string_lossy().as_ref());
    let offset = milliseconds_to_nanoseconds(correction);
    format!(
        "matroskamux name=timing_mux offset-to-zero=false ! filesink location={location} \
         audiotestsrc num-buffers=100 wave=ticks samplesperbuffer=480 timestamp-offset=0 ! audio/x-raw,rate=48000,channels=1 ! voaacenc ! aacparse ! timing_mux.audio_0 \
         audiotestsrc num-buffers=100 wave=ticks samplesperbuffer=480 timestamp-offset={offset} ! audio/x-raw,rate=48000,channels=1 ! voaacenc ! aacparse ! timing_mux.audio_1"
    )
}

#[must_use]
pub fn clean_pipeline_description(recording_pattern: &Path) -> String {
    let location = quote_property(recording_pattern.to_string_lossy().as_ref());
    format!(
        "splitmuxsink name=clean_recorder location={location} muxer-factory=matroskamux max-size-time=2000000000 send-keyframe-requests=true \
         videotestsrc is-live=true pattern=smpte ! video/x-raw,width=640,height=360,framerate=30/1 ! videoconvert ! x264enc tune=zerolatency key-int-max=30 ! h264parse ! clean_recorder.video \
         audiotestsrc is-live=true wave=ticks ! audioconvert ! audioresample ! audio/x-raw,rate=48000,channels=2 ! voaacenc ! aacparse ! clean_recorder.audio_0"
    )
}

#[must_use]
pub fn srt_input_description(uri: &str) -> String {
    let uri = quote_property(uri);
    format!(
        "srtsrc uri={uri} wait-for-connection=false ! queue max-size-time=2000000000 ! tsparse ! tsdemux name=demux \
         demux. ! queue ! h264parse ! avdec_h264 ! videoconvert ! fakesink name=decoded_video sync=false \
         demux. ! queue ! aacparse ! avdec_aac ! audioconvert ! audioresample ! fakesink name=decoded_audio sync=false"
    )
}

#[must_use]
pub fn rtmps_output_description(uri: &str) -> String {
    let uri = quote_property(uri);
    format!(
        "flvmux name=mux streamable=true ! rtmpsink location={uri} \
         videotestsrc is-live=true ! video/x-raw,width=1280,height=720,framerate=30/1 ! videoconvert ! x264enc tune=zerolatency key-int-max=60 bitrate=2500 ! h264parse ! mux. \
         audiotestsrc is-live=true ! audioconvert ! audioresample ! voaacenc bitrate=128000 ! aacparse ! mux."
    )
}

#[must_use]
pub const fn webrtc_preview_description() -> &'static str {
    "webrtcsink name=browser_preview run-signalling-server=true run-web-server=true videotestsrc is-live=true ! videoconvert ! queue max-size-buffers=1 leaky=downstream ! browser_preview.video_0 audiotestsrc is-live=true ! audioconvert ! audioresample ! queue max-size-buffers=4 leaky=downstream ! browser_preview.audio_0"
}

const fn milliseconds_to_nanoseconds(value: Milliseconds) -> i64 {
    value.value().saturating_mul(1_000_000)
}

fn quote_property(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
