use live_production_media::{
    engine::{EngineConfig, MediaEngine},
    protocol::{
        AudioMixRoute, Command, CommandId, CommandKind, EventKind, OutputId, SourceId, SourceKind,
        SourceSpec, SyntheticPattern, TrackId,
    },
};
use std::time::{Duration, Instant};

fn query(
    engine: &MediaEngine,
    id: &str,
) -> anyhow::Result<live_production_media::protocol::SourceSnapshot> {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut index = 0;
    while Instant::now() < deadline {
        index += 1;
        let event = engine.execute(Command {
            id: CommandId::new(&format!("{id}-{index}"))?,
            expected_engine_run_id: None,
            kind: CommandKind::QueryRuntime,
        })?;
        if let EventKind::RuntimeSnapshot { sources, .. } = event.kind
            && let Some(source) = sources.into_iter().next()
            && source
                .audio_input_peak_milli_db
                .is_some_and(|peak| peak > -20_000)
        {
            return Ok(source);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    anyhow::bail!("no measured source PCM")
}

#[test]
fn approved_gain_survives_reconnect_and_route_removal_physically_mutes_native_control()
-> anyhow::Result<()> {
    let engine = MediaEngine::spawn(EngineConfig::new(
        "audiomixer name=program_mixer force-live=true ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! fakesink sync=false",
        std::iter::empty::<(OutputId, &str)>(),
    ))?;
    let track = TrackId::new("audio")?;
    let source_id = SourceId::new("camera")?;
    let added = engine.execute(Command {
        id: CommandId::new("add")?,
        expected_engine_run_id: None,
        kind: CommandKind::AddSource {
            source: SourceSpec {
                id: source_id.clone(),
                video_track_id: TrackId::new("video")?,
                audio_track_id: track.clone(),
                kind: SourceKind::Synthetic {
                    pattern: SyntheticPattern::Black,
                    frequency_hz: 440,
                },
            },
        },
    })?;
    assert!(matches!(added.kind, EventKind::Applied { .. }), "{added:?}");
    assert_eq!(
        query(&engine, "initial")?.audio_post_peak_milli_db,
        Some(-120_000)
    );
    let routed = engine.execute(Command {
        id: CommandId::new("route")?,
        expected_engine_run_id: Some(added.engine_run_id.clone()),
        kind: CommandKind::ApplyAudioMix {
            routes: vec![AudioMixRoute {
                track_id: track,
                gain_milli_db: -6000,
                muted: false,
                routed: true,
            }],
        },
    })?;
    assert!(matches!(routed.kind, EventKind::Applied { .. }));
    let before = query(&engine, "approved")?;
    let difference = before
        .audio_input_peak_milli_db
        .zip(before.audio_post_peak_milli_db)
        .map(|(input, post)| input - post);
    assert!(difference.is_some_and(|difference| (difference - 6000).abs() < 20));
    let reconnected = engine.execute(Command {
        id: CommandId::new("reconnect")?,
        expected_engine_run_id: Some(added.engine_run_id.clone()),
        kind: CommandKind::ReconnectSource { source_id },
    })?;
    assert!(matches!(reconnected.kind, EventKind::Applied { .. }));
    let after = query(&engine, "restored")?;
    let difference = after
        .audio_input_peak_milli_db
        .zip(after.audio_post_peak_milli_db)
        .map(|(input, post)| input - post);
    assert!(difference.is_some_and(|difference| (difference - 6000).abs() < 20));
    let removed = engine.execute(Command {
        id: CommandId::new("unroute")?,
        expected_engine_run_id: Some(added.engine_run_id),
        kind: CommandKind::ApplyAudioMix { routes: vec![] },
    })?;
    assert!(matches!(removed.kind, EventKind::Applied { .. }));
    assert_eq!(
        query(&engine, "removed")?.audio_post_peak_milli_db,
        Some(-120_000)
    );
    Ok(())
}
