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
        "splitmuxsink name=recording_mux location={} muxer=matroskamux muxer-factory=matroskamux max-size-time={segment_ns} send-keyframe-requests=true async-finalize=false \
         intervideosrc name=recording_video_input channel={} timeout=1000000000 ! queue max-size-time=2000000000 ! videoconvert ! video/x-raw,format=I420 ! x264enc tune=zerolatency speed-preset=veryfast key-int-max=30 bitrate=2500 ! h264parse config-interval=-1 ! identity name=recording_video_timeline single-segment=true ! identity name=output_activity ! recording_mux.video \
         interaudiosrc name=recording_audio_input channel={} buffer-time=500000000 latency-time=25000000 ! queue max-size-time=2000000000 ! audioconvert ! audioresample ! audio/x-raw,rate=48000,channels=2 ! voaacenc bitrate=128000 ! aacparse ! recording_mux.audio_0",
        quote(pattern),
        tap.video_channel(),
        tap.audio_channel()
    ))
}

pub fn source_recording_pipeline(
    pattern: &str,
    segment_duration: Duration,
) -> Result<gst::Pipeline, OutputError> {
    let segment_ns = segment_duration.as_nanos().try_into().unwrap_or(u64::MAX);
    parse(&format!(
        "splitmuxsink name=recording_mux location={} muxer=matroskamux muxer-factory=matroskamux max-size-time={segment_ns} send-keyframe-requests=true async-finalize=false \
         appsrc name=recording_video_input is-live=true format=time block=false max-buffers=8 max-bytes=0 max-time=0 ! queue max-size-buffers=8 ! videoconvert ! video/x-raw,format=I420 ! x264enc tune=zerolatency speed-preset=veryfast key-int-max=30 bitrate=2500 ! h264parse config-interval=-1 ! identity name=recording_video_timeline single-segment=true ! identity name=output_activity ! recording_mux.video \
         appsrc name=recording_audio_input is-live=true format=time block=false max-buffers=64 max-bytes=0 max-time=0 ! queue max-size-buffers=64 ! audioconvert ! audioresample ! audio/x-raw,rate=48000,channels=2 ! voaacenc bitrate=128000 ! aacparse ! recording_mux.audio_0",
        quote(pattern),
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

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use anyhow::{Context, ensure};

    use super::*;

    struct PipelineGuard(gst::Pipeline);

    impl Drop for PipelineGuard {
        fn drop(&mut self) {
            let _ = self.0.set_state(gst::State::Null);
        }
    }

    #[derive(Debug)]
    struct TimelineObservation {
        segment_start: gst::ClockTime,
        pts: gst::ClockTime,
        running_time: gst::ClockTime,
    }

    #[test]
    fn recording_video_timeline_rebases_encoder_offset_before_splitmux() -> anyhow::Result<()> {
        // Given
        let id = uuid::Uuid::now_v7();
        let tap = MediaTap::new(
            &format!("timeline-video-{id}"),
            &format!("timeline-audio-{id}"),
        )?;
        let directory = tempfile::tempdir()?;
        let pattern = directory.path().join("timeline-%05d.mkv");
        let pattern = pattern.to_str().context("recording pattern")?;
        let recorder = PipelineGuard(recording_pipeline(&tap, pattern, Duration::from_secs(1))?);
        let producer = PipelineGuard(
            gst::parse::launch(&format!(
                "videotestsrc is-live=true pattern=ball ! video/x-raw,width=320,height=180,framerate=30/1 ! intervideosink channel={} sync=false \
                 audiotestsrc is-live=true wave=sine ! audio/x-raw,rate=48000,channels=2 ! interaudiosink channel={} sync=false",
                tap.video_channel(),
                tap.audio_channel()
            ))?
            .downcast::<gst::Pipeline>()
            .map_err(|_| anyhow::anyhow!("producer pipeline"))?,
        );
        let video_pad = recorder
            .0
            .by_name("recording_mux")
            .context("recording mux")?
            .static_pad("video")
            .context("recording video pad")?;
        let (observation_tx, observation_rx) = mpsc::sync_channel(1);
        video_pad.add_probe(gst::PadProbeType::BUFFER, move |pad, info| {
            let Some(pts) = info.buffer().and_then(|buffer| buffer.pts()) else {
                return gst::PadProbeReturn::Ok;
            };
            let Some(event) = pad.sticky_event::<gst::event::Segment>(0) else {
                return gst::PadProbeReturn::Ok;
            };
            let Some(segment) = event.segment().downcast_ref::<gst::ClockTime>() else {
                return gst::PadProbeReturn::Ok;
            };
            let (Some(segment_start), Some(running_time)) =
                (segment.start(), segment.to_running_time(pts))
            else {
                return gst::PadProbeReturn::Ok;
            };
            let _ = observation_tx.try_send(TimelineObservation {
                segment_start,
                pts,
                running_time,
            });
            gst::PadProbeReturn::Ok
        });

        // When
        producer.0.set_state(gst::State::Playing)?;
        recorder.0.set_state(gst::State::Playing)?;
        let observation = observation_rx
            .recv_timeout(Duration::from_secs(10))
            .context("first encoded video buffer")?;
        ensure!(recorder.0.send_event(gst::event::Eos::new()), "send EOS");
        let final_message = recorder
            .0
            .bus()
            .context("recording bus")?
            .timed_pop_filtered(
                gst::ClockTime::from_seconds(10),
                &[gst::MessageType::Eos, gst::MessageType::Error],
            )
            .context("recording finalization")?;

        // Then
        ensure!(
            matches!(final_message.view(), gst::MessageView::Eos(_)),
            "recording failed: {:?}",
            final_message.view()
        );
        let near_zero = gst::ClockTime::from_seconds(2);
        ensure!(
            observation.segment_start < near_zero,
            "segment start was {:?}",
            observation.segment_start
        );
        ensure!(observation.pts < near_zero, "PTS was {:?}", observation.pts);
        ensure!(
            observation.running_time < near_zero,
            "running time was {:?}",
            observation.running_time
        );
        Ok(())
    }

    #[test]
    fn recording_mux_finalizes_fragments_in_pipeline_order() -> anyhow::Result<()> {
        gst::init()?;
        let directory = tempfile::tempdir()?;
        let pattern = directory.path().join("ordered-%05d.mkv");
        let pattern = pattern.to_str().context("ordered pattern")?;
        let tap = MediaTap::new("ordered-video", "ordered-audio")?;
        for pipeline in [
            recording_pipeline(&tap, pattern, Duration::from_secs(1))?,
            source_recording_pipeline(pattern, Duration::from_secs(1))?,
        ] {
            let mux = pipeline.by_name("recording_mux").context("recording mux")?;
            assert!(!mux.property::<bool>("async-finalize"));
            let container = mux
                .property::<Option<gst::Element>>("muxer")
                .context("owned synchronous Matroska muxer")?;
            assert_eq!(
                container.factory().context("muxer factory")?.name(),
                "matroskamux"
            );
        }
        Ok(())
    }
}
