use live_production_media::engine::{EngineConfig, MediaEngine};
use live_production_media::protocol::{
    AudioMixRoute, Command, CommandId, CommandKind, CompositionPlacement, EventKind, FitMode,
    Milliseconds, NormalizedCrop, OutputId, OutputObservedState, PlacementContent, PlacementId,
    ProgramRevision, ScenePlacement, SourceId, SourceKind, SourceSpec, SyntheticPattern,
    TrackCorrection, TrackId,
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
fn dynamic_composition_places_one_track_twice_with_independent_transforms() -> anyhow::Result<()> {
    // Given
    let assets = tempfile::tempdir()?;
    let image_path = assets.path().join("pixel.png");
    std::fs::write(
        &image_path,
        b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x04\x00\x00\x00\xb5\x1c\x0c\x02\x00\x00\x00\x0bIDATx\xdacd\xf8\x0f\x00\x01\x05\x01\x01'\x18\xe3f\x00\x00\x00\x00IEND\xaeB`\x82",
    )?;
    let description = "compositor name=preview_compositor background=black force-live=true ignore-inactive-pads=true ! video/x-raw,width=960,height=540,framerate=30/1 ! fakesink sync=false compositor name=program_compositor background=black force-live=true ignore-inactive-pads=true ! video/x-raw,width=960,height=540,framerate=30/1 ! fakesink sync=false";
    let engine = MediaEngine::spawn(EngineConfig::new(
        description,
        std::iter::empty::<(OutputId, &str)>(),
    ))?;
    let video_track_id = TrackId::new("camera-video")?;
    let added = engine.execute(Command {
        id: CommandId::new("dynamic-source")?,
        expected_engine_run_id: None,
        kind: CommandKind::AddSource {
            source: SourceSpec {
                id: SourceId::new("camera")?,
                video_track_id: video_track_id.clone(),
                audio_track_id: TrackId::new("camera-audio")?,
                kind: SourceKind::Synthetic {
                    pattern: SyntheticPattern::Smpte,
                    frequency_hz: 440,
                },
            },
        },
    })?;
    let revision = ProgramRevision::new("dynamic-scene-1")?;
    let crop = NormalizedCrop {
        left_milli: 0,
        top_milli: 0,
        right_milli: 0,
        bottom_milli: 0,
    };
    let placements = vec![
        CompositionPlacement {
            id: PlacementId::new("camera-left")?,
            content: PlacementContent::Track {
                track_id: video_track_id.clone(),
            },
            x: 0,
            y: 0,
            width: 480,
            height: 540,
            crop,
            fit: FitMode::Cover,
            opacity_milli: 1_000,
            z_order: 0,
        },
        CompositionPlacement {
            id: PlacementId::new("camera-pip")?,
            content: PlacementContent::Track {
                track_id: video_track_id,
            },
            x: 640,
            y: 360,
            width: 320,
            height: 180,
            crop,
            fit: FitMode::Contain,
            opacity_milli: 750,
            z_order: 2,
        },
        CompositionPlacement {
            id: PlacementId::new("lower-third")?,
            content: PlacementContent::Solid { argb: 0xff204060 },
            x: 0,
            y: 480,
            width: 960,
            height: 60,
            crop,
            fit: FitMode::Stretch,
            opacity_milli: 900,
            z_order: 1,
        },
        CompositionPlacement {
            id: PlacementId::new("title")?,
            content: PlacementContent::Text {
                text: "Live".to_owned(),
                argb: 0xffff_ffff,
                font_size: 48,
            },
            x: 0,
            y: 0,
            width: 320,
            height: 90,
            crop,
            fit: FitMode::Contain,
            opacity_milli: 1_000,
            z_order: 3,
        },
        CompositionPlacement {
            id: PlacementId::new("logo")?,
            content: PlacementContent::Image {
                path: image_path.to_string_lossy().into_owned(),
            },
            x: 860,
            y: 0,
            width: 100,
            height: 100,
            crop,
            fit: FitMode::Contain,
            opacity_milli: 1_000,
            z_order: 4,
        },
    ];

    // When
    let prepared = engine.execute(Command {
        id: CommandId::new("dynamic-prepare")?,
        expected_engine_run_id: Some(added.engine_run_id.clone()),
        kind: CommandKind::PrepareComposition {
            revision: revision.clone(),
            placements,
        },
    })?;
    let taken = engine.execute(Command {
        id: CommandId::new("dynamic-take")?,
        expected_engine_run_id: Some(added.engine_run_id),
        kind: CommandKind::TakePrepared {
            revision: revision.clone(),
            expected_program_revision: None,
        },
    })?;

    // Then
    assert!(
        matches!(prepared.kind, EventKind::Applied { .. }),
        "{prepared:?}"
    );
    assert!(
        matches!(
            taken.kind,
            EventKind::Applied {
                program_revision: Some(ref applied),
                ..
            } if applied == &revision
        ),
        "{taken:?}"
    );
    let EventKind::Applied {
        media_at: Some(media_at),
        ..
    } = &taken.kind
    else {
        anyhow::bail!("Take omitted first-new-frame media PTS");
    };
    let _first_new_frame_pts = media_at.value();
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
