use std::{
    net::UdpSocket,
    ops::Deref,
    time::{Duration, Instant},
};

use gst::prelude::*;
use gstreamer as gst;
use live_production_media::{
    engine::{EngineConfig, MediaEngine},
    protocol::{
        Command, CommandId, CommandKind, EngineEvent, Milliseconds, OutputId, SourceId, SourceKind,
        SourceSpec, TrackId,
    },
};

const PASSPHRASE: &str = "epochReconnectCredential42";

pub struct PipelineGuard(pub gst::Pipeline);

impl Deref for PipelineGuard {
    type Target = gst::Pipeline;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for PipelineGuard {
    fn drop(&mut self) {
        let _ = self.0.set_state(gst::State::Null);
    }
}

pub fn engine() -> anyhow::Result<MediaEngine> {
    MediaEngine::spawn(EngineConfig::new(
        "fakesrc num-buffers=1 ! fakesink",
        std::iter::empty::<(OutputId, &str)>(),
    ))
    .map_err(Into::into)
}

pub fn add_source(engine: &MediaEngine, port: u16) -> anyhow::Result<EngineEvent> {
    engine
        .execute(Command {
            id: CommandId::new("epoch-add")?,
            expected_engine_run_id: None,
            kind: CommandKind::AddSource {
                source: SourceSpec {
                    id: SourceId::new("epoch-source")?,
                    video_track_id: TrackId::new("epoch-video")?,
                    audio_track_id: TrackId::new("epoch-audio")?,
                    kind: SourceKind::SrtListener {
                        bind_address: "127.0.0.1".into(),
                        port,
                        latency: Milliseconds::new(120),
                        passphrase: PASSPHRASE.into(),
                    },
                },
            },
        })
        .map_err(Into::into)
}

pub fn wait_admission(added: &EngineEvent, active: bool, generation: u64) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if live_production_media::engine_source_admission::snapshot(
            &added.engine_run_id,
            &SourceId::new("epoch-source")?,
        )
        .is_some_and(|value| value.active == active && value.generation == generation)
        {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    anyhow::bail!("admission state missing")
}

pub fn sender(port: u16, color: u32, frequency: u32) -> anyhow::Result<PipelineGuard> {
    let description = format!(
        "mpegtsmux name=mux alignment=7 ! srtsink uri=\"srt://127.0.0.1:{port}?mode=caller&passphrase={PASSPHRASE}&pbkeylen=16\" wait-for-connection=false auto-reconnect=false videotestsrc is-live=true pattern=solid-color foreground-color={color} ! video/x-raw,width=640,height=360,framerate=30/1 ! x264enc tune=zerolatency key-int-max=30 ! h264parse config-interval=-1 ! mux. audiotestsrc is-live=true wave=sine freq={frequency} ! audio/x-raw,rate=48000,channels=1 ! voaacenc ! aacparse ! mux."
    );
    Ok(PipelineGuard(
        gst::parse::launch(&description)?
            .downcast::<gst::Pipeline>()
            .map_err(|_| anyhow::anyhow!("sender"))?,
    ))
}

pub fn free_udp_port() -> anyhow::Result<u16> {
    let socket = UdpSocket::bind(("127.0.0.1", 0))?;
    Ok(socket.local_addr()?.port())
}
