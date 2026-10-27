use std::{
    net::UdpSocket,
    time::{Duration, Instant},
};

use gst::prelude::*;
use gstreamer as gst;
use live_production_media::{
    engine::{EngineConfig, MediaEngine},
    protocol::{
        Command, CommandId, CommandKind, EventKind, Milliseconds, OutputId, SourceId, SourceKind,
        SourceSpec, TrackCorrection, TrackId,
    },
};

struct Fixture {
    engine: MediaEngine,
    added: live_production_media::protocol::EngineEvent,
    timing: live_production_media::protocol::EngineEvent,
    port: u16,
}

#[test]
fn authenticated_srt_decode_retains_real_buffers_for_signed_timing() -> anyhow::Result<()> {
    // Given
    let fixture = fixture()?;
    let passphrase = "scopedSrtCredential42";

    // When
    let sender = srt_sender(fixture.port, passphrase)?;
    sender.set_state(gst::State::Playing)?;
    let connected = wait_for_history(&fixture.engine, &fixture.added)?;
    sender.set_state(gst::State::Null)?;
    std::thread::sleep(Duration::from_secs(4));
    let disconnected = runtime_snapshot(&fixture.engine, &fixture.added, "network-disconnected")?;

    // Then
    assert!(matches!(
        fixture.timing.kind,
        EventKind::Applied {
            discontinuity: true,
            ..
        }
    ));
    assert!(connected.connected);
    let scheduled_lead = connected
        .video_playout_pts
        .ok_or_else(|| anyhow::anyhow!("playout PTS missing"))?
        .value()
        .saturating_sub(
            connected
                .video_received_pts
                .ok_or_else(|| anyhow::anyhow!("received PTS missing"))?
                .value(),
        );
    let retained_video = 250_000_000_u64.saturating_sub(scheduled_lead);
    assert!(
        (180_000_000..=283_000_000).contains(&retained_video),
        "decoded retained video ns={retained_video}"
    );
    assert!(connected.audio_received_buffers.value() > connected.audio_buffers.value());
    assert!(!disconnected.connected);
    assert!(
        disconnected
            .gaps
            .last()
            .is_some_and(|gap| gap.ended_at.is_none())
    );
    Ok(())
}

#[test]
fn wrong_srt_secret_never_produces_decoded_buffers() -> anyhow::Result<()> {
    // Given
    let fixture = fixture()?;
    let sender = srt_sender(fixture.port, "wrongScopedCredential99")?;

    // When
    sender.set_state(gst::State::Playing)?;
    std::thread::sleep(Duration::from_secs(1));
    sender.set_state(gst::State::Null)?;
    let rejected = runtime_snapshot(&fixture.engine, &fixture.added, "wrong-secret-snapshot")?;

    // Then
    assert_eq!(rejected.video_received_frames.value(), 0);
    assert_eq!(rejected.audio_received_buffers.value(), 0);
    Ok(())
}

fn fixture() -> anyhow::Result<Fixture> {
    gst::init()?;
    let port = free_udp_port()?;
    let engine = MediaEngine::spawn(EngineConfig::new(
        "fakesrc num-buffers=1 ! fakesink",
        std::iter::empty::<(OutputId, &str)>(),
    ))?;
    let added = engine.execute(Command {
        id: CommandId::new("network-add")?,
        expected_engine_run_id: None,
        kind: CommandKind::AddSource {
            source: SourceSpec {
                id: SourceId::new("network-camera")?,
                video_track_id: TrackId::new("network-video")?,
                audio_track_id: TrackId::new("network-audio")?,
                kind: SourceKind::SrtListener {
                    bind_address: "127.0.0.1".to_owned(),
                    port,
                    latency: Milliseconds::new(120),
                    passphrase: "scopedSrtCredential42".to_owned(),
                },
            },
        },
    })?;
    let timing = apply_timing(&engine, &added, "network-timing")?;
    Ok(Fixture {
        engine,
        added,
        timing,
        port,
    })
}

fn apply_timing(
    engine: &MediaEngine,
    added: &live_production_media::protocol::EngineEvent,
    id: &str,
) -> anyhow::Result<live_production_media::protocol::EngineEvent> {
    engine
        .execute(Command {
            id: CommandId::new(id)?,
            expected_engine_run_id: Some(added.engine_run_id.clone()),
            kind: CommandKind::ApplyTrackTiming {
                program_holdback: Milliseconds::new(500),
                corrections: vec![
                    TrackCorrection {
                        track_id: TrackId::new("network-video")?,
                        correction: Milliseconds::new(-250),
                    },
                    TrackCorrection {
                        track_id: TrackId::new("network-audio")?,
                        correction: Milliseconds::new(125),
                    },
                ],
            },
        })
        .map_err(Into::into)
}

fn wait_for_history(
    engine: &MediaEngine,
    added: &live_production_media::protocol::EngineEvent,
) -> anyhow::Result<live_production_media::protocol::SourceSnapshot> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut sequence = 0_u32;
    let mut last = None;
    while Instant::now() < deadline {
        sequence = sequence.saturating_add(1);
        let source = runtime_snapshot(engine, added, &format!("network-snapshot-{sequence}"))?;
        let retained = source
            .video_received_pts
            .zip(source.video_playout_pts)
            .map_or(0, |(received, played)| {
                250_000_000_u64.saturating_sub(played.value().saturating_sub(received.value()))
            });
        if source.connected
            && (180_000_000..=283_000_000).contains(&retained)
            && source.audio_received_buffers.value() > source.audio_buffers.value()
        {
            return Ok(source);
        }
        last = Some(source);
        std::thread::sleep(Duration::from_millis(25));
    }
    anyhow::bail!("SRT source did not retain decoded timing history: {last:?}")
}

fn runtime_snapshot(
    engine: &MediaEngine,
    added: &live_production_media::protocol::EngineEvent,
    command_id: &str,
) -> anyhow::Result<live_production_media::protocol::SourceSnapshot> {
    let event = engine.execute(Command {
        id: CommandId::new(command_id)?,
        expected_engine_run_id: Some(added.engine_run_id.clone()),
        kind: CommandKind::QueryRuntime,
    })?;
    let EventKind::RuntimeSnapshot { sources, .. } = event.kind else {
        anyhow::bail!("expected runtime snapshot");
    };
    sources
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("source missing"))
}

fn free_udp_port() -> anyhow::Result<u16> {
    let socket = UdpSocket::bind(("127.0.0.1", 0))?;
    Ok(socket.local_addr()?.port())
}

fn srt_sender(port: u16, passphrase: &str) -> anyhow::Result<gst::Pipeline> {
    let description = format!(
        "mpegtsmux name=mux ! srtsink uri=\"srt://127.0.0.1:{port}?mode=caller&passphrase={passphrase}&pbkeylen=16\" wait-for-connection=true \
         videotestsrc is-live=true pattern=ball ! video/x-raw,width=640,height=360,framerate=30/1 ! videoconvert ! x264enc tune=zerolatency key-int-max=30 bitrate=1000 ! h264parse config-interval=-1 ! mux. \
         audiotestsrc is-live=true wave=ticks ! audioconvert ! audioresample ! audio/x-raw,rate=48000,channels=2 ! voaacenc bitrate=128000 ! aacparse ! mux."
    );
    gst::parse::launch(&description)?
        .downcast::<gst::Pipeline>()
        .map_err(|_| anyhow::anyhow!("sender description did not create a pipeline"))
}
