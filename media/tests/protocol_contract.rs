use live_production_media::protocol::{
    Command, CommandId, CommandKind, EngineEvent, EngineRunId, EventKind, Milliseconds,
    Nanoseconds, OutputId, OutputObservedState, OutputSnapshot, Sequence,
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
