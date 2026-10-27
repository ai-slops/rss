use live_production_media::engine::{EngineConfig, MediaEngine};
use live_production_media::protocol::{
    AudioMixRoute, Command, CommandId, CommandKind, EventKind, Milliseconds, OutputId,
    OutputObservedState, ProgramRevision, ScenePlacement, TrackCorrection, TrackId,
};

#[test]
fn owner_thread_applies_timing_and_controls_output_valve() -> anyhow::Result<()> {
    // Given
    let output_id = OutputId::new("preview")?;
    let config = EngineConfig::new(
        "videotestsrc name=video_timing_source is-live=true ! valve name=preview_valve drop=true ! fakesink sync=false audiotestsrc name=audio_timing_source is-live=true ! fakesink sync=false",
        [(output_id.clone(), "preview_valve")],
    );
    let engine = MediaEngine::spawn(config)?;

    // When
    let timing = engine.execute(Command {
        id: CommandId::new("timing-1")?,
        expected_engine_run_id: None,
        kind: CommandKind::ApplyTiming {
            program_holdback: Milliseconds::new(500),
            video_correction: Milliseconds::new(-250),
            audio_correction: Milliseconds::new(125),
        },
    })?;
    let started = engine.execute(Command {
        id: CommandId::new("start-1")?,
        expected_engine_run_id: Some(timing.engine_run_id.clone()),
        kind: CommandKind::StartOutput { output_id },
    })?;

    // Then
    assert!(matches!(timing.kind, EventKind::Applied { .. }));
    assert!(matches!(started.kind, EventKind::Applied { .. }));
    let snapshot = engine.execute(Command {
        id: CommandId::new("snapshot-1")?,
        expected_engine_run_id: Some(timing.engine_run_id),
        kind: CommandKind::QuerySnapshot,
    })?;
    let EventKind::Snapshot { outputs } = snapshot.kind else {
        anyhow::bail!("expected snapshot event");
    };
    assert_eq!(
        outputs.first().map(|output| output.observed),
        Some(OutputObservedState::Running)
    );
    Ok(())
}

#[test]
fn owner_applies_live_audio_gain_mute_and_route() -> anyhow::Result<()> {
    // Given
    let track_id = TrackId::new("microphone")?;
    let config = EngineConfig::new(
        "audiomixer name=program_mixer ! fakesink sync=false audiotestsrc is-live=true ! program_mixer.sink_0",
        std::iter::empty::<(OutputId, &str)>(),
    )
    .with_audio_route(track_id.clone(), "sink_0");
    let engine = MediaEngine::spawn(config)?;

    // When
    let event = engine.execute(Command {
        id: CommandId::new("mix-1")?,
        expected_engine_run_id: None,
        kind: CommandKind::ApplyAudioMix {
            routes: vec![AudioMixRoute {
                track_id,
                gain_milli_db: -6_000,
                muted: false,
                routed: true,
            }],
        },
    })?;

    // Then
    assert!(matches!(event.kind, EventKind::Applied { .. }));
    Ok(())
}

#[test]
fn owner_prepares_preview_takes_exact_revision_and_applies_track_timing() -> anyhow::Result<()> {
    // Given
    let description = "compositor name=preview_compositor ! fakesink sync=false compositor name=program_compositor ! fakesink sync=false videotestsrc name=track_source is-live=true ! tee name=source source. ! queue ! preview_compositor.sink_0 source. ! queue ! program_compositor.sink_0";
    let track_id = TrackId::new("camera-video")?;
    let engine = MediaEngine::spawn(
        EngineConfig::new(description, std::iter::empty::<(OutputId, &str)>())
            .with_track(track_id.clone(), "track_source"),
    )?;
    let revision = ProgramRevision::new("scene-7")?;
    let placement = ScenePlacement {
        slot: 0,
        x: 40,
        y: 20,
        width: 320,
        height: 180,
        opacity_milli: 900,
    };

    // When
    let timing = engine.execute(Command {
        id: CommandId::new("track-timing-1")?,
        expected_engine_run_id: None,
        kind: CommandKind::ApplyTrackTiming {
            program_holdback: Milliseconds::new(500),
            corrections: vec![TrackCorrection {
                track_id,
                correction: Milliseconds::new(-250),
            }],
        },
    })?;
    let prepared = engine.execute(Command {
        id: CommandId::new("prepare-1")?,
        expected_engine_run_id: Some(timing.engine_run_id.clone()),
        kind: CommandKind::PreparePreview {
            revision: revision.clone(),
            placements: vec![placement],
        },
    })?;
    let taken = engine.execute(Command {
        id: CommandId::new("take-1")?,
        expected_engine_run_id: Some(timing.engine_run_id),
        kind: CommandKind::TakePrepared {
            revision: revision.clone(),
            expected_program_revision: None,
        },
    })?;

    // Then
    assert!(matches!(timing.kind, EventKind::Applied { .. }));
    assert!(matches!(prepared.kind, EventKind::Applied { .. }));
    assert!(matches!(
        taken.kind,
        EventKind::Applied {
            program_revision: Some(ref applied),
            ..
        } if applied == &revision
    ));
    Ok(())
}
