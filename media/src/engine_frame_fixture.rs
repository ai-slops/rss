use anyhow::Context;
use gstreamer::{self as gst, prelude::*};
use gstreamer_app::AppSink;

pub struct Fixture {
    pub pipeline: gst::Pipeline,
    pub compositor: gst::Element,
    sink: AppSink,
}

impl Fixture {
    pub fn new(rate: u32) -> anyhow::Result<Self> {
        gst::init()?;
        let pipeline = gst::parse::launch(&format!(
            "compositor name=mix force-live=true background=black sink_0::alpha=1 sink_1::alpha=0 ! video/x-raw,format=RGB,width=16,height=16,framerate={rate}/1 ! appsink name=frames max-buffers=32 drop=true sync=false videotestsrc is-live=true pattern=solid-color foreground-color=4294901760 ! video/x-raw,format=RGB,width=16,height=16,framerate={rate}/1 ! mix.sink_0 videotestsrc is-live=true pattern=solid-color foreground-color=4278255360 ! video/x-raw,format=RGB,width=16,height=16,framerate={rate}/1 ! mix.sink_1"
        ))?.downcast::<gst::Pipeline>().map_err(|_| anyhow::anyhow!("fixture pipeline"))?;
        let compositor = pipeline.by_name("mix").context("compositor")?;
        let sink = pipeline
            .by_name("frames")
            .context("frame sink")?
            .downcast::<AppSink>()
            .map_err(|_| anyhow::anyhow!("appsink type"))?;
        pipeline.set_state(gst::State::Playing)?;
        Ok(Self {
            pipeline,
            compositor,
            sink,
        })
    }

    pub fn frame(&self) -> anyhow::Result<(u64, Vec<u8>)> {
        let sample = self
            .sink
            .try_pull_sample(gst::ClockTime::from_seconds(2))
            .context("decoded frame deadline")?;
        let buffer = sample.buffer().context("frame buffer")?;
        let pts = buffer.pts().context("frame PTS")?.nseconds();
        let pixels = buffer.map_readable()?;
        Ok((
            pts,
            pixels.as_slice().get(..3).context("RGB pixel")?.to_vec(),
        ))
    }

    pub fn pad(&self, pad: &str) -> anyhow::Result<gst::Pad> {
        self.compositor.static_pad(pad).context("compositor pad")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}
