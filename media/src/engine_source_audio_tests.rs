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

#[test]
fn direct_source_mix_delivers_continuous_pcm_with_program_holdback() -> anyhow::Result<()> {
    use crate::{
        engine_source_runtime::{SourceBuild, SourceBuildContext, SourceRuntime},
        source_capture::SourceCaptureRegistry,
    };
    gst::init()?;
    let run = EngineRunId::new(&uuid::Uuid::now_v7().to_string())?;
    let port = std::net::UdpSocket::bind("127.0.0.1:0")?
        .local_addr()?
        .port();
    let sender = gst::parse::launch(&format!("mpegtsmux name=mux alignment=7 ! srtsink uri=\"srt://127.0.0.1:{port}?mode=caller&passphrase=ContinuousTest123&pbkeylen=16\" wait-for-connection=true videotestsrc is-live=true ! video/x-raw,width=320,height=180,framerate=60/1 ! x264enc tune=zerolatency ! h264parse ! mux. audiotestsrc is-live=true freq=440 ! audio/x-raw,rate=48000,channels=2 ! voaacenc ! aacparse ! mux."))?.downcast::<gst::Pipeline>().map_err(|_| anyhow::anyhow!("sender"))?;
    let spec = SourceSpec {
        id: SourceId::new("continuous")?,
        video_track_id: TrackId::new("continuous-video")?,
        audio_track_id: TrackId::new("continuous-audio")?,
        kind: SourceKind::SrtListener {
            bind_address: "127.0.0.1".into(),
            port,
            latency: crate::protocol::Milliseconds::new(120),
            passphrase: "ContinuousTest123".into(),
        },
    };
    let master = gst::parse::launch("audiomixer name=program_mixer force-live=true ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! appsink name=master max-buffers=64 drop=true sync=false")?.downcast::<gst::Pipeline>().map_err(|_| anyhow::anyhow!("master"))?;
    let captures = SourceCaptureRegistry::default();
    master.set_state(gst::State::Playing)?;
    let result = (|| -> anyhow::Result<()> {
        let runtime = SourceRuntime::build(
            &SourceBuildContext {
                pipeline: &master,
                engine_run_id: &run,
                source_captures: &captures,
            },
            SourceBuild {
                spec,
                generation: 1,
                gaps: vec![],
            },
        )
        .map_err(anyhow::Error::msg)?;
        sender.set_state(gst::State::Playing)?;
        runtime.audio_timing_control.set_offset(500_000_000);
        runtime
            .audio_mix
            .as_ref()
            .context("direct route")?
            .mixer_pad
            .set_property("mute", false);
        let sink = master
            .by_name("master")
            .context("sink")?
            .downcast::<AppSink>()
            .map_err(|_| anyhow::anyhow!("sink type"))?;
        let mut silent = 0;
        for i in 0..300 {
            let sample = sink
                .try_pull_sample(gst::ClockTime::from_seconds(2))
                .context("PCM sample")?;
            if i >= 100 && peak(sample.buffer().context("PCM")?)? < 100 {
                silent += 1;
            }
        }
        runtime.detach(&master).map_err(anyhow::Error::msg)?;
        anyhow::ensure!(
            silent < 5,
            "Program contained {silent}/200 silent audio blocks after warmup"
        );
        Ok(())
    })();
    sender.set_state(gst::State::Null)?;
    master.set_state(gst::State::Null)?;
    result
}
