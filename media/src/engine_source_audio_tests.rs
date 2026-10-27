use super::*;
use crate::protocol::{SourceId, SourceKind, SyntheticPattern, TrackId};
use anyhow::Context;
use gstreamer_app::AppSink;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

struct AudioFixture {
    producer: gst::Pipeline,
    master: gst::Pipeline,
    route: Option<AudioMixRuntime>,
    sink: AppSink,
}

impl Drop for AudioFixture {
    fn drop(&mut self) {
        if let Some(route) = self.route.take() {
            let _ = route.detach(&self.master);
        }
        let _ = self.master.set_state(gst::State::Null);
        let _ = self.producer.set_state(gst::State::Null);
    }
}

fn peak(buffer: &gst::BufferRef) -> anyhow::Result<u16> {
    let bytes = buffer.map_readable()?;
    Ok(bytes
        .as_slice()
        .chunks_exact(2)
        .map(|sample| i16::from_le_bytes([sample[0], sample[1]]).unsigned_abs())
        .max()
        .unwrap_or(0))
}

#[test]
fn unapproved_new_audio_is_physically_silent_until_explicitly_routed() -> anyhow::Result<()> {
    gst::init()?;
    let run = EngineRunId::new(&uuid::Uuid::now_v7().to_string())?;
    let spec = SourceSpec {
        id: SourceId::new("mic")?,
        video_track_id: TrackId::new("video")?,
        audio_track_id: TrackId::new("audio")?,
        kind: SourceKind::Synthetic {
            pattern: SyntheticPattern::Black,
            frequency_hz: 440,
        },
    };
    let channel = TapChannels::source(&run, &spec.video_track_id, &spec.audio_track_id).audio;
    let producer = gst::parse::launch(&format!("audiotestsrc is-live=true wave=sine freq=440 volume=0.5 ! audioconvert ! audioresample ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! interaudiosink channel={channel} sync=false async=false"))?.downcast::<gst::Pipeline>().map_err(|_| anyhow::anyhow!("producer"))?;
    let master = gst::parse::launch("audiomixer name=program_mixer force-live=true ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! appsink name=master max-buffers=64 drop=true sync=false")?.downcast::<gst::Pipeline>().map_err(|_| anyhow::anyhow!("master"))?;
    producer.set_state(gst::State::Playing)?;
    master.set_state(gst::State::Playing)?;
    let route = AudioMixRuntime::build(&master, &spec, &run)
        .map_err(anyhow::Error::msg)?
        .context("route")?;
    let sink = master
        .by_name("master")
        .context("master sink")?
        .downcast::<AppSink>()
        .map_err(|_| anyhow::anyhow!("appsink"))?;
    let input_ready = Arc::new(AtomicBool::new(false));
    let ready = Arc::clone(&input_ready);
    route
        .bin
        .static_pad("src")
        .context("source audio")?
        .add_probe(gst::PadProbeType::BUFFER, move |_, info| {
            if info
                .buffer()
                .and_then(|buffer| peak(buffer).ok())
                .is_some_and(|peak| peak > 100)
            {
                ready.store(true, Ordering::Relaxed);
                return gst::PadProbeReturn::Remove;
            }
            gst::PadProbeReturn::Ok
        })
        .context("observe real pre-fader samples")?;
    let fixture = AudioFixture {
        producer,
        master,
        route: Some(route),
        sink,
    };
    let deadline = Instant::now() + Duration::from_secs(3);
    while !input_ready.load(Ordering::Relaxed) {
        anyhow::ensure!(
            Instant::now() < deadline,
            "tone did not reach pre-fader input"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let mut unapproved_peak = 0;
    for _ in 0..32 {
        let sample = fixture
            .sink
            .try_pull_sample(gst::ClockTime::from_seconds(1))
            .context("master sample")?;
        unapproved_peak = unapproved_peak.max(peak(sample.buffer().context("PCM")?)?);
    }
    assert_eq!(
        unapproved_peak, 0,
        "new source audio leaked into Program before explicit route approval"
    );
    fixture
        .route
        .as_ref()
        .context("route")?
        .mixer_pad
        .set_property("mute", false);
    let mut approved_peak = 0;
    for _ in 0..32 {
        let sample = fixture
            .sink
            .try_pull_sample(gst::ClockTime::from_seconds(1))
            .context("routed sample")?;
        approved_peak = approved_peak.max(peak(sample.buffer().context("PCM")?)?);
    }
    assert!(
        approved_peak > 1000,
        "approved route did not deliver real PCM"
    );
    Ok(())
}
