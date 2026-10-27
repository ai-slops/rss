use live_production_media::engine::{EngineConfig, MediaEngine};
use live_production_media::protocol::{
    Command, CommandId, CommandKind, EventKind, Milliseconds, OutputId, SourceId, SourceKind,
    SourceSpec, SyntheticPattern, TrackCorrection, TrackId,
};

#[test]
fn source_reconnect_preserves_ids_and_opens_new_continuity_generation() -> anyhow::Result<()> {
    // Given
    let source_id = SourceId::new("camera-a")?;
    let video_track_id = TrackId::new("camera-a-video")?;
    let audio_track_id = TrackId::new("camera-a-audio")?;
    let engine = MediaEngine::spawn(EngineConfig::new(
        "fakesrc num-buffers=1 ! fakesink",
        std::iter::empty::<(OutputId, &str)>(),
    ))?;
    let added = engine.execute(Command {
        id: CommandId::new("source-add")?,
        expected_engine_run_id: None,
        kind: CommandKind::AddSource {
            source: SourceSpec {
                id: source_id.clone(),
                video_track_id: video_track_id.clone(),
                audio_track_id,
                kind: SourceKind::Synthetic {
                    pattern: SyntheticPattern::Ball,
                    frequency_hz: 660,
                },
            },
        },
    })?;
    let timing = engine.execute(Command {
        id: CommandId::new("source-timing")?,
        expected_engine_run_id: Some(added.engine_run_id.clone()),
        kind: CommandKind::ApplyTrackTiming {
            program_holdback: Milliseconds::new(500),
            corrections: vec![TrackCorrection {
                track_id: video_track_id,
                correction: Milliseconds::new(-250),
            }],
        },
    })?;

    // When
    let reconnected = engine.execute(Command {
        id: CommandId::new("source-reconnect")?,
        expected_engine_run_id: Some(added.engine_run_id.clone()),
        kind: CommandKind::ReconnectSource {
            source_id: source_id.clone(),
        },
    })?;
    let snapshot = engine.execute(Command {
        id: CommandId::new("source-snapshot")?,
        expected_engine_run_id: Some(added.engine_run_id),
        kind: CommandKind::QueryRuntime,
    })?;

    // Then
    assert!(matches!(added.kind, EventKind::Applied { .. }));
    assert!(matches!(
        timing.kind,
        EventKind::Applied {
            discontinuity: true,
            ..
        }
    ));
    assert!(matches!(
        reconnected.kind,
        EventKind::Applied {
            discontinuity: true,
            ..
        }
    ));
    let EventKind::RuntimeSnapshot { sources, .. } = snapshot.kind else {
        anyhow::bail!("expected runtime snapshot");
    };
    let source = sources
        .first()
        .ok_or_else(|| anyhow::anyhow!("source missing"))?;
    assert_eq!(source.id, source_id);
    assert_eq!(source.continuity_generation.value(), 2);
    assert_eq!(source.gaps.len(), 1);
    Ok(())
}

#[test]
fn srt_source_rejects_unscoped_passphrase_before_graph_mutation() -> anyhow::Result<()> {
    // Given
    let engine = MediaEngine::spawn(EngineConfig::new(
        "fakesrc num-buffers=1 ! fakesink",
        std::iter::empty::<(OutputId, &str)>(),
    ))?;

    // When
    let event = engine.execute(Command {
        id: CommandId::new("srt-add")?,
        expected_engine_run_id: None,
        kind: CommandKind::AddSource {
            source: SourceSpec {
                id: SourceId::new("remote-a")?,
                video_track_id: TrackId::new("remote-a-video")?,
                audio_track_id: TrackId::new("remote-a-audio")?,
                kind: SourceKind::SrtListener {
                    port: 9100,
                    latency: Milliseconds::new(120),
                    passphrase: "short".to_owned(),
                },
            },
        },
    })?;

    // Then
    assert!(matches!(event.kind, EventKind::Rejected { .. }));
    Ok(())
}

#[test]
fn rejected_source_configuration_keeps_previous_generation_running() -> anyhow::Result<()> {
    // Given
    let source_id = SourceId::new("stable-camera")?;
    let video_track_id = TrackId::new("stable-video")?;
    let audio_track_id = TrackId::new("stable-audio")?;
    let engine = MediaEngine::spawn(EngineConfig::new(
        "fakesrc num-buffers=1 ! fakesink",
        std::iter::empty::<(OutputId, &str)>(),
    ))?;
    let added = engine.execute(Command {
        id: CommandId::new("stable-add")?,
        expected_engine_run_id: None,
        kind: CommandKind::AddSource {
            source: SourceSpec {
                id: source_id.clone(),
                video_track_id: video_track_id.clone(),
                audio_track_id: audio_track_id.clone(),
                kind: SourceKind::Synthetic {
                    pattern: SyntheticPattern::Smpte,
                    frequency_hz: 440,
                },
            },
        },
    })?;

    // When
    let configured = engine.execute(Command {
        id: CommandId::new("stable-configure")?,
        expected_engine_run_id: Some(added.engine_run_id.clone()),
        kind: CommandKind::ConfigureSource {
            source: SourceSpec {
                id: source_id.clone(),
                video_track_id,
                audio_track_id,
                kind: SourceKind::SrtListener {
                    port: 9_100,
                    latency: Milliseconds::new(120),
                    passphrase: "short".to_owned(),
                },
            },
        },
    })?;
    let snapshot = engine.execute(Command {
        id: CommandId::new("stable-snapshot")?,
        expected_engine_run_id: Some(added.engine_run_id),
        kind: CommandKind::QueryRuntime,
    })?;

    // Then
    assert!(matches!(configured.kind, EventKind::Rejected { .. }));
    let EventKind::RuntimeSnapshot { sources, .. } = snapshot.kind else {
        anyhow::bail!("expected runtime snapshot");
    };
    assert_eq!(sources[0].id, source_id);
    assert_eq!(sources[0].continuity_generation.value(), 1);
    Ok(())
}
