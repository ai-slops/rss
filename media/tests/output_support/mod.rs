use std::time::{Duration, Instant};

use gst::prelude::*;
use gstreamer as gst;
use live_production_media::output::{MediaTap, OutputId, OutputManager};

pub fn producer(tap: &MediaTap) -> anyhow::Result<gst::Pipeline> {
    let description = format!(
        "videotestsrc is-live=true pattern=ball ! video/x-raw,width=320,height=180,framerate=30/1 ! intervideosink name=fixture_video_tap channel={} sync=false \
         audiotestsrc is-live=true wave=ticks ! audio/x-raw,rate=48000,channels=2 ! interaudiosink channel={} sync=false",
        tap.video_channel(),
        tap.audio_channel()
    );
    gst::parse::launch(&description)?
        .downcast::<gst::Pipeline>()
        .map_err(|_| anyhow::anyhow!("producer description did not create a pipeline"))
}

pub fn wait_for_media(manager: &OutputManager, id: &OutputId) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if manager.snapshot(id)?.last_media_at_ns.is_some() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    anyhow::bail!("output produced no media")
}
