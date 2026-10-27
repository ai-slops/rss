mod fixture;

use std::time::Duration;

use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app::AppSink;
use live_production_media::{protocol::TrackId, tap::TapChannels};

use fixture::{PipelineGuard, add_source, free_udp_port, sender, wait_admission};

pub struct ReconnectEvidence {
    pub queued_old_video: bool,
    pub queued_old_audio: bool,
    pub first_video_pts: u64,
    pub second_video_pts: u64,
    pub first_audio_pts: u64,
    pub second_audio_pts: u64,
    pub second_video_marker: u32,
    pub second_video_generation: u64,
    pub second_audio_marker: u32,
    pub second_audio_generation: u64,
}

struct Evidence {
    marker: u32,
    generation: u64,
    pts: u64,
}

pub fn reconnect_evidence() -> anyhow::Result<ReconnectEvidence> {
    gst::init()?;
    let port = free_udp_port()?;
    let engine = fixture::engine()?;
    let added = add_source(&engine, port)?;
    let channels = TapChannels::clean(
        &added.engine_run_id,
        &TrackId::new("epoch-video")?,
        &TrackId::new("epoch-audio")?,
    );
    let (video_pipeline, video_sink) = video_consumer(&channels.video)?;
    let (audio_pipeline, audio_sink) = audio_consumer(&channels.audio)?;
    video_pipeline.set_state(gst::State::Playing)?;
    audio_pipeline.set_state(gst::State::Playing)?;
    let first = sender(port, 0xffff_0000, 440)?;
    first.set_state(gst::State::Playing)?;
    let first_video = wait_video(&video_sink, 1, 0xffff_0000)?;
    let first_audio = wait_audio(&audio_sink, 1, 440)?;
    std::thread::sleep(Duration::from_millis(300));
    first.set_state(gst::State::Null)?;
    wait_admission(&added, false, 1)?;
    let second = sender(port, 0xff00_00ff, 880)?;
    second.set_state(gst::State::Playing)?;
    wait_admission(&added, true, 2)?;
    let (queued_old_video, second_video) = pull_until_video(&video_sink, first_video.pts)?;
    let (queued_old_audio, second_audio) = pull_until_audio(&audio_sink, first_audio.pts)?;
    Ok(ReconnectEvidence {
        queued_old_video,
        queued_old_audio,
        first_video_pts: first_video.pts,
        second_video_pts: second_video.pts,
        first_audio_pts: first_audio.pts,
        second_audio_pts: second_audio.pts,
        second_video_marker: second_video.marker,
        second_video_generation: second_video.generation,
        second_audio_marker: second_audio.marker,
        second_audio_generation: second_audio.generation,
    })
}

fn pull_until_video(sink: &AppSink, previous_pts: u64) -> anyhow::Result<(bool, Evidence)> {
    let mut queued = false;
    let mut last = previous_pts;
    for _ in 0..64 {
        let Some(value) = video_evidence(sink)? else {
            continue;
        };
        anyhow::ensure!(value.pts >= last, "video PTS regressed");
        last = value.pts;
        if value.marker == 0xffff_0000 {
            anyhow::ensure!(value.generation == 1, "queued red relabeled");
            queued = true;
        }
        if value.marker == 0xff00_00ff {
            return Ok((queued, value));
        }
    }
    anyhow::bail!("blue reconnect frame missing")
}

fn pull_until_audio(sink: &AppSink, previous_pts: u64) -> anyhow::Result<(bool, Evidence)> {
    let mut queued = false;
    let mut last = previous_pts;
    for _ in 0..128 {
        let Some(value) = audio_evidence(sink)? else {
            continue;
        };
        anyhow::ensure!(value.pts >= last, "audio PTS regressed");
        last = value.pts;
        if (400..=480).contains(&value.marker) {
            anyhow::ensure!(value.generation == 1, "queued audio relabeled");
            queued = true;
        }
        if (820..=940).contains(&value.marker) {
            return Ok((queued, value));
        }
    }
    anyhow::bail!("880 Hz reconnect audio missing")
}

fn wait_video(sink: &AppSink, generation: u64, marker: u32) -> anyhow::Result<Evidence> {
    for _ in 0..128 {
        if let Some(value) = video_evidence(sink)?
            && value.generation == generation
            && value.marker == marker
        {
            return Ok(value);
        }
    }
    anyhow::bail!("video marker missing")
}

fn wait_audio(sink: &AppSink, generation: u64, marker: u32) -> anyhow::Result<Evidence> {
    for _ in 0..128 {
        if let Some(value) = audio_evidence(sink)?
            && value.generation == generation
            && value.marker.abs_diff(marker) <= 40
        {
            return Ok(value);
        }
    }
    anyhow::bail!("audio marker missing")
}

fn video_evidence(sink: &AppSink) -> anyhow::Result<Option<Evidence>> {
    let sample = sample(sink)?;
    let buffer = sample
        .buffer()
        .ok_or_else(|| anyhow::anyhow!("video buffer"))?;
    let map = buffer.map_readable()?;
    let marker = match &map.as_slice()[1_382_400..1_382_403] {
        [r, g, b] if *r > 180 && *g < 80 && *b < 80 => 0xffff_0000,
        [r, g, b] if *b > 180 && *r < 80 && *g < 80 => 0xff00_00ff,
        _ => 0,
    };
    evidence(buffer, marker)
}

fn audio_evidence(sink: &AppSink) -> anyhow::Result<Option<Evidence>> {
    let sample = sample(sink)?;
    let buffer = sample
        .buffer()
        .ok_or_else(|| anyhow::anyhow!("audio buffer"))?;
    let map = buffer.map_readable()?;
    let samples = map
        .as_slice()
        .chunks_exact(2)
        .map(|v| i16::from_le_bytes([v[0], v[1]]))
        .collect::<Vec<_>>();
    let crossings = samples
        .windows(2)
        .filter(|v| v[0].is_negative() != v[1].is_negative())
        .count();
    evidence(
        buffer,
        u32::try_from(crossings.saturating_mul(24_000) / samples.len().max(1))?,
    )
}

fn evidence(buffer: &gst::BufferRef, marker: u32) -> anyhow::Result<Option<Evidence>> {
    let Some(meta) = buffer.iter_meta::<gst::ReferenceTimestampMeta>().find(|m| {
        m.reference().structure(0).is_some_and(|v| {
            v.name() == "timestamp/x-live-production-engine" && v.has_field("continuity-generation")
        })
    }) else {
        return Ok(None);
    };
    let reference = meta
        .reference()
        .structure(0)
        .ok_or_else(|| anyhow::anyhow!("reference"))?;
    let _ = reference.get::<&str>("source-id")?;
    let _ = reference.get::<&str>("track-id")?;
    let _ = reference.get::<&str>("track-kind")?;
    Ok(Some(Evidence {
        marker,
        generation: reference.get::<u64>("continuity-generation")?,
        pts: buffer
            .pts()
            .ok_or_else(|| anyhow::anyhow!("PTS"))?
            .nseconds(),
    }))
}

fn sample(sink: &AppSink) -> anyhow::Result<gst::Sample> {
    sink.try_pull_sample(gst::ClockTime::from_seconds(2))
        .ok_or_else(|| anyhow::anyhow!("sample timeout"))
}
fn video_consumer(c: &str) -> anyhow::Result<(PipelineGuard, AppSink)> {
    consumer(&format!(
        "intervideosrc channel=\"{c}\" timeout=1000000000 ! videoconvert ! video/x-raw,format=RGB,width=1280,height=720,framerate=30/1"
    ))
}
fn audio_consumer(c: &str) -> anyhow::Result<(PipelineGuard, AppSink)> {
    consumer(&format!(
        "interaudiosrc channel=\"{c}\" ! audioconvert ! audio/x-raw,format=S16LE,rate=48000,channels=1"
    ))
}
fn consumer(prefix: &str) -> anyhow::Result<(PipelineGuard, AppSink)> {
    let pipeline = gst::parse::launch(&format!(
        "{prefix} ! appsink name=sink max-buffers=32 drop=true sync=false"
    ))?
    .downcast::<gst::Pipeline>()
    .map_err(|_| anyhow::anyhow!("consumer"))?;
    let sink = pipeline
        .by_name("sink")
        .ok_or_else(|| anyhow::anyhow!("sink"))?
        .downcast::<AppSink>()
        .map_err(|_| anyhow::anyhow!("appsink"))?;
    Ok((PipelineGuard(pipeline), sink))
}
