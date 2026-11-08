use std::{
    ops::Deref,
    sync::{Arc, Barrier},
    thread::JoinHandle,
    time::{Duration, Instant},
};

use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app::AppSink;
use live_production_media::engine_source_admission::publisher_stream_id;

use super::PASSPHRASE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    Red,
    Green,
    Blue,
    Other,
}

pub struct PipelineGuard(gst::Pipeline);

impl PipelineGuard {
    pub fn start(&self) -> anyhow::Result<()> {
        self.0.set_state(gst::State::Playing)?;
        Ok(())
    }

    pub fn stop(&self) -> anyhow::Result<()> {
        self.0.set_state(gst::State::Null)?;
        let (_, state, _) = self.0.state(gst::ClockTime::from_seconds(5));
        anyhow::ensure!(state == gst::State::Null, "pipeline did not reach NULL");
        Ok(())
    }
}

impl Deref for PipelineGuard {
    type Target = gst::Pipeline;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for PipelineGuard {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

pub fn consumer(channel: &str) -> anyhow::Result<(PipelineGuard, AppSink)> {
    pipeline_with_sink(&format!(
        "intervideosrc channel=\"{channel}\" timeout=1000000000 ! videoconvert ! video/x-raw,format=RGB,width=1280,height=720,framerate=30/1 ! appsink name=sink max-buffers=8 drop=true sync=false"
    ))
}

pub fn audio_consumer(channel: &str) -> anyhow::Result<(PipelineGuard, AppSink)> {
    pipeline_with_sink(&format!(
        "interaudiosrc channel=\"{channel}\" ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! appsink name=sink max-buffers=16 drop=true sync=false"
    ))
}

pub fn wait_for_color(sink: &AppSink, expected: Color) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        if pull_color(sink)? == expected {
            return Ok(());
        }
    }
    anyhow::bail!("never observed {expected:?}")
}

pub fn wait_for_color_generation(
    sink: &AppSink,
    expected: Color,
    generation: u64,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        let sample = pull_sample(sink)?;
        if color(&sample)? == expected && sample_generation(&sample) == Some(generation) {
            return Ok(());
        }
    }
    anyhow::bail!("never observed {expected:?} generation {generation}")
}

pub fn wait_for_audio_generation(sink: &AppSink, generation: u64) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        if sample_generation(&pull_sample(sink)?) == Some(generation) {
            return Ok(());
        }
    }
    anyhow::bail!("never observed audio generation {generation}")
}

pub fn wait_for_authoritative_color(sink: &AppSink) -> anyhow::Result<Color> {
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        let observed = pull_color(sink)?;
        if matches!(observed, Color::Red | Color::Green) {
            return Ok(observed);
        }
    }
    anyhow::bail!("no authoritative publisher color appeared")
}

pub fn assert_samples_remain(sink: &AppSink, expected: Color, count: usize) -> anyhow::Result<()> {
    for _ in 0..count {
        let observed = pull_color(sink)?;
        anyhow::ensure!(observed == expected, "publisher changed to {observed:?}");
    }
    Ok(())
}

pub fn sender(port: u16, color: u32, frequency: u32) -> anyhow::Result<PipelineGuard> {
    sender_with_stream_id(
        port,
        color,
        frequency,
        PASSPHRASE,
        &publisher_stream_id(PASSPHRASE),
    )
}

pub fn provisioned_stream_id() -> String {
    publisher_stream_id(PASSPHRASE)
}

pub fn sender_with_stream_id(
    port: u16,
    color: u32,
    frequency: u32,
    passphrase: &str,
    stream_id: &str,
) -> anyhow::Result<PipelineGuard> {
    let description = format!(
        "mpegtsmux name=mux alignment=7 ! srtsink uri=\"srt://127.0.0.1:{port}?mode=caller&passphrase={passphrase}&pbkeylen=16&streamid={stream_id}\" wait-for-connection=false auto-reconnect=false \
         videotestsrc is-live=true pattern=solid-color foreground-color={color} ! video/x-raw,width=640,height=360,framerate=30/1 ! x264enc tune=zerolatency key-int-max=30 ! h264parse config-interval=-1 ! mux. \
         audiotestsrc is-live=true wave=sine freq={frequency} ! audio/x-raw,rate=48000,channels=2 ! voaacenc ! aacparse ! mux."
    );
    gst::parse::launch(&description)?
        .downcast::<gst::Pipeline>()
        .map(PipelineGuard)
        .map_err(|_| anyhow::anyhow!("sender is not a pipeline"))
}

pub fn start_on_barrier(
    sender: PipelineGuard,
    barrier: Arc<Barrier>,
) -> JoinHandle<Result<PipelineGuard, String>> {
    std::thread::spawn(move || {
        barrier.wait();
        sender.start().map_err(|error| error.to_string())?;
        Ok(sender)
    })
}

pub fn join_sender(
    handle: JoinHandle<Result<PipelineGuard, String>>,
) -> anyhow::Result<PipelineGuard> {
    handle
        .join()
        .map_err(|_| anyhow::anyhow!("sender thread panicked"))?
        .map_err(anyhow::Error::msg)
}

fn pipeline_with_sink(description: &str) -> anyhow::Result<(PipelineGuard, AppSink)> {
    let pipeline = gst::parse::launch(description)?
        .downcast::<gst::Pipeline>()
        .map_err(|_| anyhow::anyhow!("consumer is not a pipeline"))?;
    let sink = pipeline
        .by_name("sink")
        .ok_or_else(|| anyhow::anyhow!("consumer sink missing"))?
        .downcast::<AppSink>()
        .map_err(|_| anyhow::anyhow!("consumer sink has wrong type"))?;
    Ok((PipelineGuard(pipeline), sink))
}

fn pull_color(sink: &AppSink) -> anyhow::Result<Color> {
    color(&pull_sample(sink)?)
}

fn pull_sample(sink: &AppSink) -> anyhow::Result<gst::Sample> {
    sink.try_pull_sample(gst::ClockTime::from_seconds(2))
        .ok_or_else(|| anyhow::anyhow!("timed out waiting for source sample"))
}

fn color(sample: &gst::Sample) -> anyhow::Result<Color> {
    let buffer = sample
        .buffer()
        .ok_or_else(|| anyhow::anyhow!("frame has no buffer"))?;
    let map = buffer.map_readable()?;
    let pixel = &map.as_slice()[1_382_400..1_382_403];
    Ok(match pixel {
        [r, g, b] if *r > 180 && *g < 80 && *b < 80 => Color::Red,
        [r, g, b] if *g > 180 && *r < 80 && *b < 80 => Color::Green,
        [r, g, b] if *b > 180 && *r < 80 && *g < 80 => Color::Blue,
        _ => Color::Other,
    })
}

fn sample_generation(sample: &gst::Sample) -> Option<u64> {
    sample
        .buffer()?
        .iter_meta::<gst::ReferenceTimestampMeta>()
        .filter_map(|meta| {
            let structure = meta.reference().structure(0)?;
            (structure.name() == "timestamp/x-live-production-engine")
                .then(|| structure.get::<u64>("continuity-generation").ok())?
        })
        .last()
}
