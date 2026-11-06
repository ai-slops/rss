use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

use live_production_media::{
    engine::{EngineConfig, MediaEngine},
    output::{
        MediaTap, OutputDefinition, OutputId as ManagedOutputId, OutputManager,
        RecordingDefinition, RecordingMapping, RecordingSegment,
    },
    protocol::{
        Command, CommandId, CommandKind, EventKind, Nanoseconds, OutputId, SourceId, SourceKind,
        SourceSpec, SyntheticPattern, TrackId,
    },
    tap::TapChannels,
};

#[test]
fn already_live_ingest_behind_clock_records_with_exact_common_origin() -> anyhow::Result<()> {
    // Given
    let engine = MediaEngine::spawn(EngineConfig::new(
        "fakesrc num-buffers=1 ! fakesink",
        std::iter::empty::<(OutputId, &str)>(),
    ))?;
    let run = engine.execute(Command {
        id: CommandId::new("lag-query")?,
        expected_engine_run_id: None,
        kind: CommandKind::QuerySnapshot,
    })?;
    let video_track = TrackId::new("lag-video")?;
    let audio_track = TrackId::new("lag-audio")?;
    let source_id = SourceId::new("lag-source")?;
    let added = engine.execute(Command {
        id: CommandId::new("lag-add")?,
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
    std::thread::sleep(Duration::from_millis(600));
    let channels = TapChannels::clean(&run.engine_run_id, &video_track, &audio_track);
    let capture = engine.source_capture(&channels);
    let runtime = engine.execute(Command {
        id: CommandId::new("lag-runtime")?,
        expected_engine_run_id: Some(run.engine_run_id.clone()),
        kind: CommandKind::QueryRuntime,
    })?;
    let EventKind::RuntimeSnapshot { mut clock, .. } = runtime.kind else {
        anyhow::bail!("missing runtime clock");
    };
    clock.media_time = Nanoseconds::new(clock.media_time.value().saturating_add(500_000_000));
    let advanced_anchor = clock.media_time.value();
    let manager = OutputManager::new();
    let output_id = ManagedOutputId::new("lag-source-capture")?;
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

    // When
    let start = Instant::now();
    manager.start_with_clock(&output_id, Some(clock))?;
    assert!(start.elapsed() < Duration::from_secs(1));
    std::thread::sleep(Duration::from_secs(2));
    manager.stop(&output_id)?;

    // Then
    let segments = manager.recordings(&output_id)?;
    assert_recordings(
        &segments,
        &source_id,
        &video_track,
        &audio_track,
        advanced_anchor,
    );
    Ok(())
}

fn assert_recordings(
    segments: &[RecordingSegment],
    source_id: &SourceId,
    video_track: &TrackId,
    audio_track: &TrackId,
    advanced_anchor: u64,
) {
    assert!(!segments.is_empty());
    assert!(segments.iter().all(|value| value.decoded));
    assert!(
        segments
            .iter()
            .all(|value| value.mapping == RecordingMapping::Mapped)
    );
    let intervals = segments
        .iter()
        .flat_map(|segment| &segment.provenance)
        .collect::<Vec<_>>();
    let by_track = intervals.iter().fold(
        BTreeMap::<String, BTreeSet<u64>>::new(),
        |mut values, interval| {
            assert_eq!(interval.source_id, source_id.as_str());
            values
                .entry(interval.track_id.clone())
                .or_default()
                .insert(interval.continuity_generation);
            values
        },
    );
    assert_eq!(
        by_track.get(video_track.as_str()),
        Some(&BTreeSet::from([1]))
    );
    assert_eq!(
        by_track.get(audio_track.as_str()),
        Some(&BTreeSet::from([1]))
    );
    let origins = intervals
        .iter()
        .map(|value| value.media_start_ns - value.local_start_ns)
        .collect::<BTreeSet<_>>();
    assert_eq!(origins.len(), 1);
    assert!(
        origins
            .first()
            .is_some_and(|origin| *origin < advanced_anchor)
    );
    eprintln!(
        "lag recording: anchor={advanced_anchor} origin={} segments={} video_start={} audio_start={}",
        origins.iter().next().copied().unwrap_or_default(),
        segments.len(),
        intervals
            .iter()
            .find_map(
                |value| (value.track_id == video_track.as_str()).then_some(value.media_start_ns)
            )
            .unwrap_or_default(),
        intervals
            .iter()
            .find_map(
                |value| (value.track_id == audio_track.as_str()).then_some(value.media_start_ns)
            )
            .unwrap_or_default(),
    );
}
