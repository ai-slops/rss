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

#[test]
fn authenticated_srt_decode_retains_real_buffers_for_signed_timing() -> anyhow::Result<()> {
    // Given
    gst::init()?;
    let port = free_udp_port()?;
    let passphrase = "scopedSrtCredential42";
    let video_track = TrackId::new("network-video")?;
    let audio_track = TrackId::new("network-audio")?;
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
                video_track_id: video_track.clone(),
                audio_track_id: audio_track.clone(),
                kind: SourceKind::SrtListener {
                    port,
                    latency: Milliseconds::new(120),
                    passphrase: passphrase.to_owned(),
                },
            },
        },
    })?;
    let timing = engine.execute(Command {
        id: CommandId::new("network-timing")?,
        expected_engine_run_id: Some(added.engine_run_id.clone()),
        kind: CommandKind::ApplyTrackTiming {
            program_holdback: Milliseconds::new(500),
            corrections: vec![
                TrackCorrection {
                    track_id: video_track,
                    correction: Milliseconds::new(-250),
                },
                TrackCorrection {
                    track_id: audio_track,
                    correction: Milliseconds::new(125),
                },
            ],
        },
    })?;

    // When: a publisher with the wrong secret must produce no decoded buffers.
    let wrong_sender = srt_sender(port, "wrongScopedCredential99")?;
    wrong_sender.set_state(gst::State::Playing)?;
    std::thread::sleep(Duration::from_secs(1));
    wrong_sender.set_state(gst::State::Null)?;
    let rejected = runtime_snapshot(&engine, &added, "wrong-secret-snapshot")?;
    assert_eq!(rejected.video_received_frames.value(), 0);
    assert_eq!(rejected.audio_received_buffers.value(), 0);
    let reconnected = engine.execute(Command {
        id: CommandId::new("network-reconnect-after-rejection")?,
        expected_engine_run_id: Some(added.engine_run_id.clone()),
        kind: CommandKind::ReconnectSource {
            source_id: SourceId::new("network-camera")?,
        },
    })?;
    assert!(matches!(reconnected.kind, EventKind::Applied { .. }));
    let reapplied = engine.execute(Command {
        id: CommandId::new("network-timing-after-reconnect")?,
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
    })?;
    assert!(matches!(reapplied.kind, EventKind::Applied { .. }));

    let sender = srt_sender(port, passphrase)?;
    sender.set_state(gst::State::Playing)?;
    let connected = wait_for_history(&engine, &added)?;
    sender.set_state(gst::State::Null)?;
    std::thread::sleep(Duration::from_millis(2_200));
    let disconnected = runtime_snapshot(&engine, &added, "network-disconnected")?;

    // Then
    assert!(matches!(
        timing.kind,
        EventKind::Applied {
            discontinuity: true,
            ..
        }
    ));
    assert!(connected.connected);
    let retained_video = connected
        .video_received_frames
        .value()
        .saturating_sub(connected.video_frames.value());
    assert!(
        (5..=10).contains(&retained_video),
        "decoded retained video buffers={retained_video}"
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
            .video_received_frames
            .value()
            .saturating_sub(source.video_frames.value());
        if source.connected
            && retained >= 5
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
