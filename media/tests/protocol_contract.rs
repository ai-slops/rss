use live_production_media::protocol::{
    Command, CommandId, CommandKind, CompositionPlacement, EngineEvent, EngineRunId, EventKind,
    FitMode, Milliseconds, Nanoseconds, NormalizedCrop, OutputId, OutputObservedState,
    OutputSnapshot, PlacementContent, PlacementId, ProgramRevision, Sequence, SourceId, SourceKind,
    SourceSpec, SyntheticPattern, TrackId,
};
use serde_json::json;

#[test]
fn command_round_trips_when_timing_is_signed() -> anyhow::Result<()> {
    // Given
    let command = Command {
        id: CommandId::new("cmd-17")?,
        expected_engine_run_id: Some(EngineRunId::new("run-4")?),
        kind: CommandKind::ApplyTiming {
            program_holdback: Milliseconds::new(500),
            video_correction: Milliseconds::new(-250),
            audio_correction: Milliseconds::new(125),
        },
    };

    // When
    let encoded = serde_json::to_value(&command)?;
    let decoded: Command = serde_json::from_value(encoded)?;

    // Then
    assert_eq!(decoded, command);
    Ok(())
}

#[test]
fn native_tap_channels_are_scoped_by_engine_run() -> anyhow::Result<()> {
    // Given
    let first = EngineRunId::new("run-first")?;
    let second = EngineRunId::new("run-second")?;

    // When
    let first_program = live_production_media::tap::TapChannels::program(&first);
    let second_program = live_production_media::tap::TapChannels::program(&second);

    // Then
    assert_ne!(first_program, second_program);
    assert!(first_program.video.contains(first.as_str()));
    assert!(second_program.audio.contains(second.as_str()));
    Ok(())
}

#[test]
fn dynamic_source_and_composition_round_trip_with_stable_ids() -> anyhow::Result<()> {
    // Given
    let source = Command {
        id: CommandId::new("source-add-1")?,
        expected_engine_run_id: None,
        kind: CommandKind::AddSource {
            source: SourceSpec {
                id: SourceId::new("camera-a")?,
                video_track_id: TrackId::new("camera-a-video")?,
                audio_track_id: TrackId::new("camera-a-audio")?,
                kind: SourceKind::Synthetic {
                    pattern: SyntheticPattern::Ball,
                    frequency_hz: 660,
                },
            },
        },
    };
    let composition = Command {
        id: CommandId::new("prepare-dynamic-1")?,
        expected_engine_run_id: None,
        kind: CommandKind::PrepareComposition {
            revision: ProgramRevision::new("scene-dynamic-2")?,
            placements: vec![CompositionPlacement {
                id: PlacementId::new("placement-a")?,
                content: PlacementContent::Track {
                    track_id: TrackId::new("camera-a-video")?,
                },
                x: 12,
                y: 24,
                width: 640,
                height: 360,
                crop: NormalizedCrop {
                    left_milli: 100,
                    top_milli: 0,
                    right_milli: 50,
                    bottom_milli: 0,
                },
                fit: FitMode::Cover,
                opacity_milli: 800,
                z_order: 3,
            }],
        },
    };

    // When
    let source_round_trip: Command = serde_json::from_value(serde_json::to_value(&source)?)?;
    let composition_round_trip: Command =
        serde_json::from_value(serde_json::to_value(&composition)?)?;

    // Then
    assert_eq!(source_round_trip, source);
    assert_eq!(composition_round_trip, composition);
    Ok(())
}

#[test]
fn event_uses_decimal_string_timestamp_when_value_exceeds_javascript_integer_precision()
-> anyhow::Result<()> {
    // Given
    let event = EngineEvent {
        sequence: Sequence::new(9_007_199_254_740_993),
        engine_run_id: EngineRunId::new("run-4")?,
        command_id: Some(CommandId::new("cmd-18")?),
        at: Nanoseconds::new(9_007_199_254_740_993),
        kind: EventKind::Snapshot {
            outputs: vec![OutputSnapshot {
                id: OutputId::new("program-recording")?,
                desired_running: true,
                observed: OutputObservedState::Running,
            }],
        },
    };

    // When
    let encoded = serde_json::to_value(event)?;

    // Then
    assert_eq!(encoded["sequence"], json!("9007199254740993"));
    assert_eq!(encoded["at_ns"], json!("9007199254740993"));
    Ok(())
}

#[test]
fn identifier_rejects_whitespace_when_parsed_at_boundary() {
    // Given
    let raw = "program output";

    // When
    let result = OutputId::new(raw);

    // Then
    assert!(result.is_err());
}
