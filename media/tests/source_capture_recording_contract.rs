use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use live_production_media::{
    engine::{EngineConfig, MediaEngine},
    output::{
        MediaTap, OutputDefinition, OutputId as ManagedOutputId, OutputManager, RecordingDefinition,
    },
    protocol::{
        Command, CommandId, CommandKind, EventKind, OutputId, SourceId, SourceKind, SourceSpec,
        SyntheticPattern, TrackId,
    },
    tap::TapChannels,
};

#[test]
fn causal_source_capture_records_both_reconnect_generations_without_consumer_metadata()
-> anyhow::Result<()> {
    // Given
    let engine = MediaEngine::spawn(EngineConfig::new(
        "fakesrc num-buffers=1 ! fakesink",
        std::iter::empty::<(OutputId, &str)>(),
    ))?;
    let run = engine.execute(Command {
        id: CommandId::new("capture-query")?,
        expected_engine_run_id: None,
        kind: CommandKind::QuerySnapshot,
    })?;
    let video_track = TrackId::new("capture-video")?;
    let audio_track = TrackId::new("capture-audio")?;
    let channels = TapChannels::clean(&run.engine_run_id, &video_track, &audio_track);
    let capture = engine.source_capture(&channels);
    let source_id = SourceId::new("capture-source")?;
    let added = engine.execute(Command {
        id: CommandId::new("capture-add")?,
        expected_engine_run_id: Some(run.engine_run_id.clone()),
        kind: CommandKind::AddSource {
            source: SourceSpec {
                id: source_id.clone(),
                video_track_id: video_track.clone(),
                audio_track_id: audio_track.clone(),
                kind: SourceKind::Synthetic {
                    pattern: SyntheticPattern::Black,
                    frequency_hz: 440,
                },
            },
        },
    })?;
    assert!(matches!(added.kind, EventKind::Applied { .. }));
    let runtime = engine.execute(Command {
        id: CommandId::new("capture-runtime")?,
        expected_engine_run_id: Some(run.engine_run_id.clone()),
        kind: CommandKind::QueryRuntime,
    })?;
    let EventKind::RuntimeSnapshot { clock, .. } = runtime.kind else {
        anyhow::bail!("missing runtime clock");
    };
    let manager = OutputManager::new();
    let output_id = ManagedOutputId::new("causal-source-capture")?;
    let directory = tempfile::tempdir()?;
    manager.register(OutputDefinition::Recording(
        RecordingDefinition::new(
            output_id.clone(),
            MediaTap::new(&channels.video, &channels.audio)?,
            directory.path(),
            Duration::from_secs(1),
        )?
        .with_engine_run_id(&run.engine_run_id)
        .with_source_capture(capture),
    ))?;
    manager.start_with_clock(&output_id, Some(clock))?;
    std::thread::sleep(Duration::from_millis(1_200));

    // When
    let reconnected = engine.execute(Command {
        id: CommandId::new("capture-reconnect")?,
        expected_engine_run_id: Some(run.engine_run_id),
        kind: CommandKind::ReconnectSource { source_id },
    })?;
    assert!(matches!(reconnected.kind, EventKind::Applied { .. }));
    std::thread::sleep(Duration::from_millis(1_500));
    let stop_started = std::time::Instant::now();
    manager.stop(&output_id)?;
    assert!(stop_started.elapsed() < Duration::from_secs(2));

    // Then
    let segments = manager.recordings(&output_id)?;
    assert!(!segments.is_empty());
    assert!(segments.iter().all(|segment| segment.decoded));
    let generations = segments
        .into_iter()
        .flat_map(|segment| segment.provenance)
        .fold(
            BTreeMap::<String, BTreeSet<u64>>::new(),
            |mut values, interval| {
                assert_eq!(interval.source_id, "capture-source");
                values
                    .entry(interval.track_id)
                    .or_default()
                    .insert(interval.continuity_generation);
                values
            },
        );
    assert_eq!(
        generations.get(video_track.as_str()),
        Some(&BTreeSet::from([1, 2]))
    );
    assert_eq!(
        generations.get(audio_track.as_str()),
        Some(&BTreeSet::from([1, 2]))
    );
    Ok(())
}
