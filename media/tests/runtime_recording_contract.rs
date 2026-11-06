use std::time::{Duration, Instant};

mod native_evidence;

use live_production_media::{
    engine::{EngineConfig, MediaEngine},
    output::{
        MediaTap, OutputDefinition, OutputId as ManagedOutputId, OutputManager, OutputState,
        PROGRAM_AUDIO_CHANNEL, PROGRAM_VIDEO_CHANNEL, RecordingDefinition,
    },
    protocol::{
        Command, CommandId, CommandKind, CompositionPlacement, EventKind, FitMode, NormalizedCrop,
        OutputId, PlacementContent, PlacementId, ProgramRevision, SourceId, SourceKind, SourceSpec,
        SyntheticPattern, TrackId,
    },
};

#[test]
fn dynamic_take_records_real_program_without_restarting_consumer() -> anyhow::Result<()> {
    // Given
    let caps_diagnostic =
        std::env::var("LIVE_PRODUCTION_FIXED_CAPS_DIAGNOSTIC").unwrap_or_default();
    let pipeline = match caps_diagnostic.as_str() {
        "1" => program_pipeline().replace(
            "video/x-raw,format=I420,width=960,height=540,framerate=30/1",
            "video/x-raw,format=I420,colorimetry=bt601,chroma-site=jpeg,width=960,height=540,framerate=30/1",
        ),
        "legacy" => program_pipeline().replace(
            "video/x-raw,format=I420,width=960,height=540,framerate=30/1",
            "video/x-raw,width=960,height=540,framerate=30/1",
        ),
        "" | "0" | "format" => program_pipeline().to_owned(),
        _ => anyhow::bail!("unknown caps diagnostic profile"),
    };
    eprintln!("caps_diagnostic_profile={caps_diagnostic}");
    let engine = MediaEngine::spawn(EngineConfig::new(
        &pipeline,
        std::iter::empty::<(OutputId, &str)>(),
    ))?;
    let video_track = TrackId::new("camera-a-video")?;
    let source = engine.execute(Command {
        id: CommandId::new("record-source")?,
        expected_engine_run_id: None,
        kind: CommandKind::AddSource {
            source: SourceSpec {
                id: SourceId::new("camera-a")?,
                video_track_id: video_track.clone(),
                audio_track_id: TrackId::new("camera-a-audio")?,
                kind: SourceKind::Synthetic {
                    pattern: SyntheticPattern::Ball,
                    frequency_hz: 440,
                },
            },
        },
    })?;
    let manager = OutputManager::new();
    let output_id = ManagedOutputId::new("runtime-program-recording")?;
    let directory = tempfile::tempdir()?;
    manager.register(OutputDefinition::Recording(RecordingDefinition::new(
        output_id.clone(),
        MediaTap::new(PROGRAM_VIDEO_CHANNEL, PROGRAM_AUDIO_CHANNEL)?,
        directory.path(),
        Duration::from_secs(1),
    )?))?;
    manager.start(&output_id)?;
    wait_for_state(&manager, &output_id, OutputState::Running)?;
    let first_revision = ProgramRevision::new("runtime-scene-1")?;
    take(
        &engine,
        &source,
        &first_revision,
        placement(&video_track, "camera-full", 0, 0, 960, 540)?,
        None,
    )?;
    wait_for_media(&manager, &output_id)?;

    // When
    let second_revision = ProgramRevision::new("runtime-scene-2")?;
    take(
        &engine,
        &source,
        &second_revision,
        placement(&video_track, "camera-pip", 600, 320, 320, 180)?,
        Some(first_revision),
    )?;
    std::thread::sleep(Duration::from_secs(3));
    manager.stop(&output_id)?;
    wait_for_state(&manager, &output_id, OutputState::Stopped)?;

    // Then
    let segments = manager.recordings(&output_id)?;
    native_evidence::preserve(directory.path(), "runtime-recording")?;
    assert!(
        segments.len() >= 2,
        "expected bounded Program segments: {segments:?}"
    );
    assert!(segments.iter().all(|segment| segment.bytes > 0));
    assert!(segments.iter().all(|segment| segment.decoded));
    Ok(())
}

fn take(
    engine: &MediaEngine,
    source: &live_production_media::protocol::EngineEvent,
    revision: &ProgramRevision,
    placement: CompositionPlacement,
    expected: Option<ProgramRevision>,
) -> anyhow::Result<()> {
    let prepared = engine.execute(Command {
        id: CommandId::new(&format!("prepare-{}", revision.as_str()))?,
        expected_engine_run_id: Some(source.engine_run_id.clone()),
        kind: CommandKind::PrepareComposition {
            revision: revision.clone(),
            placements: vec![placement],
        },
    })?;
    let taken = engine.execute(Command {
        id: CommandId::new(&format!("take-{}", revision.as_str()))?,
        expected_engine_run_id: Some(source.engine_run_id.clone()),
        kind: CommandKind::TakePrepared {
            revision: revision.clone(),
            expected_program_revision: expected,
        },
    })?;
    if !matches!(prepared.kind, EventKind::Applied { .. })
        || !matches!(taken.kind, EventKind::Applied { .. })
    {
        anyhow::bail!("dynamic Take failed: prepared={prepared:?} taken={taken:?}");
    }
    Ok(())
}

fn placement(
    track_id: &TrackId,
    id: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> anyhow::Result<CompositionPlacement> {
    Ok(CompositionPlacement {
        id: PlacementId::new(id)?,
        content: PlacementContent::Track {
            track_id: track_id.clone(),
        },
        x,
        y,
        width,
        height,
        crop: NormalizedCrop {
            left_milli: 0,
            top_milli: 0,
            right_milli: 0,
            bottom_milli: 0,
        },
        fit: FitMode::Cover,
        opacity_milli: 1_000,
        z_order: 0,
    })
}

const fn program_pipeline() -> &'static str {
    "compositor name=preview_compositor background=black force-live=true ignore-inactive-pads=true ! video/x-raw,format=I420,width=960,height=540,framerate=30/1 ! fakesink sync=false \
     compositor name=program_compositor background=black force-live=true ignore-inactive-pads=true ! video/x-raw,format=I420,width=960,height=540,framerate=30/1 ! tee name=program_video \
     program_video. ! queue max-size-buffers=2 leaky=downstream ! intervideosink name=program_video_tap channel=live-program-video-v1 sync=false async=false \
     program_video. ! queue max-size-buffers=2 leaky=downstream ! fakesink sync=false \
     audiomixer name=program_mixer force-live=true ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! tee name=program_audio \
     program_audio. ! queue max-size-time=200000000 leaky=downstream ! interaudiosink name=program_audio_tap channel=live-program-audio-v1 sync=false async=false \
     program_audio. ! queue max-size-time=200000000 leaky=downstream ! fakesink sync=false"
}

fn wait_for_state(
    manager: &OutputManager,
    id: &ManagedOutputId,
    state: OutputState,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if manager.snapshot(id)?.actual == state {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    anyhow::bail!("output did not reach {state:?}")
}

fn wait_for_media(manager: &OutputManager, id: &ManagedOutputId) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if manager.snapshot(id)?.last_media_at_ns.is_some() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    anyhow::bail!("recording received no Program media")
}
