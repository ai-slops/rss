use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use gst::prelude::*;
use gstreamer as gst;

use crate::{
    engine::{EngineConfig, MediaEngine},
    output::{
        MediaTap, OutputDefinition, OutputId as ManagedOutputId, OutputSnapshot, OutputState,
        RecordingDefinition,
    },
    output_owner::SharedOutput,
    output_runtime::ActivePipeline,
    output_runtime_support::progress_units,
    protocol::{
        Command, CommandId, CommandKind, EventKind, OutputId, SourceId, SourceKind, SourceSpec,
        SyntheticPattern, TrackId,
    },
    tap::TapChannels,
};

#[test]
fn source_recording_delivers_one_eos_per_mux_track() -> anyhow::Result<()> {
    // Given
    let engine = MediaEngine::spawn(EngineConfig::new(
        "fakesrc num-buffers=1 ! fakesink",
        std::iter::empty::<(OutputId, &str)>(),
    ))?;
    let run = engine.execute(Command {
        id: CommandId::new("eos-query")?,
        expected_engine_run_id: None,
        kind: CommandKind::QuerySnapshot,
    })?;
    let video_track = TrackId::new("eos-video")?;
    let audio_track = TrackId::new("eos-audio")?;
    let added = engine.execute(Command {
        id: CommandId::new("eos-add")?,
        expected_engine_run_id: Some(run.engine_run_id.clone()),
        kind: CommandKind::AddSource {
            source: SourceSpec {
                id: SourceId::new("eos-source")?,
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
    std::thread::sleep(Duration::from_millis(300));
    let channels = TapChannels::clean(&run.engine_run_id, &video_track, &audio_track);
    let runtime = engine.execute(Command {
        id: CommandId::new("eos-runtime")?,
        expected_engine_run_id: Some(run.engine_run_id.clone()),
        kind: CommandKind::QueryRuntime,
    })?;
    let EventKind::RuntimeSnapshot { clock, .. } = runtime.kind else {
        anyhow::bail!("missing runtime clock");
    };
    let directory = tempfile::tempdir()?;
    let definition = RecordingDefinition::new(
        ManagedOutputId::new("eos-recording")?,
        MediaTap::new(&channels.video, &channels.audio)?,
        directory.path(),
        Duration::from_secs(1),
    )?
    .with_engine_run_id(&run.engine_run_id)
    .with_source_capture(engine.source_capture(&channels));
    let output = OutputDefinition::Recording(definition);
    let mut active = ActivePipeline::start(&output, Some(&clock))?;
    let video_eos = Arc::new(AtomicUsize::new(0));
    let audio_eos = Arc::new(AtomicUsize::new(0));
    observe_eos(&active.pipeline, "video", Arc::clone(&video_eos))?;
    observe_eos(&active.pipeline, "audio_0", Arc::clone(&audio_eos))?;
    active.last_progress_units = wait_for_progress(&active.pipeline)?;
    let shared = shared_output();

    // When
    active.stop(&output, &shared)?;

    // Then
    assert_eq!(video_eos.load(Ordering::Acquire), 1);
    assert_eq!(audio_eos.load(Ordering::Acquire), 1);
    assert_strict_matroska(&shared)?;
    Ok(())
}

#[test]
fn program_recording_drains_eos_from_both_live_inputs() -> anyhow::Result<()> {
    // Given
    gst::init()?;
    let id = uuid::Uuid::now_v7();
    let tap = MediaTap::new(
        &format!("eos-program-video-{id}"),
        &format!("eos-program-audio-{id}"),
    )?;
    let producer = gst::parse::launch(&format!(
        "videotestsrc is-live=true pattern=black ! video/x-raw,width=320,height=180,framerate=30/1 ! intervideosink channel={} sync=false async=false \
         audiotestsrc is-live=true wave=sine ! audio/x-raw,rate=48000,channels=2 ! interaudiosink channel={} sync=false async=false",
        tap.video_channel(),
        tap.audio_channel()
    ))?
    .downcast::<gst::Pipeline>()
    .map_err(|_| anyhow::anyhow!("producer pipeline"))?;
    let directory = tempfile::tempdir()?;
    let output = OutputDefinition::Recording(RecordingDefinition::new(
        ManagedOutputId::new("eos-program-recording")?,
        tap,
        directory.path(),
        Duration::from_secs(1),
    )?);
    let mut active = ActivePipeline::start(&output, None)?;
    let video_eos = Arc::new(AtomicUsize::new(0));
    let audio_eos = Arc::new(AtomicUsize::new(0));
    observe_source_eos(
        &active.pipeline,
        "recording_video_input",
        Arc::clone(&video_eos),
    )?;
    observe_source_eos(
        &active.pipeline,
        "recording_audio_input",
        Arc::clone(&audio_eos),
    )?;
    producer.set_state(gst::State::Playing)?;
    active.last_progress_units = wait_for_progress(&active.pipeline)?;
    let shared = shared_output();

    // When
    active.stop(&output, &shared)?;

    // Then
    assert_eq!(video_eos.load(Ordering::Acquire), 1);
    assert_eq!(audio_eos.load(Ordering::Acquire), 1);
    assert_strict_matroska(&shared)?;
    producer.set_state(gst::State::Null)?;
    Ok(())
}

fn observe_eos(
    pipeline: &gst::Pipeline,
    pad_name: &str,
    counter: Arc<AtomicUsize>,
) -> anyhow::Result<()> {
    pipeline
        .by_name("recording_mux")
        .and_then(|mux| mux.static_pad(pad_name))
        .ok_or_else(|| anyhow::anyhow!("mux pad {pad_name}"))?
        .add_probe(gst::PadProbeType::EVENT_DOWNSTREAM, move |_, info| {
            if info
                .event()
                .is_some_and(|event| matches!(event.view(), gst::EventView::Eos(_)))
            {
                counter.fetch_add(1, Ordering::AcqRel);
            }
            gst::PadProbeReturn::Ok
        })
        .ok_or_else(|| anyhow::anyhow!("EOS probe {pad_name}"))?;
    Ok(())
}

fn assert_strict_matroska(shared: &Mutex<SharedOutput>) -> anyhow::Result<()> {
    let recordings = shared
        .lock()
        .map_err(|_| anyhow::anyhow!("output state lock"))?
        .recordings
        .clone();
    anyhow::ensure!(!recordings.is_empty(), "no finalized fragments");
    for recording in &recordings {
        anyhow::ensure!(recording.decoded, "fragment failed strict decode");
        let bytes = std::fs::read(&recording.path)?;
        anyhow::ensure!(
            bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]),
            "not EBML/Matroska"
        );
    }
    Ok(())
}

fn observe_source_eos(
    pipeline: &gst::Pipeline,
    source_name: &str,
    counter: Arc<AtomicUsize>,
) -> anyhow::Result<()> {
    pipeline
        .by_name(source_name)
        .and_then(|source| source.static_pad("src"))
        .ok_or_else(|| anyhow::anyhow!("source pad {source_name}"))?
        .add_probe(gst::PadProbeType::EVENT_DOWNSTREAM, move |_, info| {
            if info
                .event()
                .is_some_and(|event| matches!(event.view(), gst::EventView::Eos(_)))
            {
                counter.fetch_add(1, Ordering::AcqRel);
            }
            gst::PadProbeReturn::Ok
        })
        .ok_or_else(|| anyhow::anyhow!("source EOS probe {source_name}"))?;
    Ok(())
}

fn wait_for_progress(pipeline: &gst::Pipeline) -> anyhow::Result<u64> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let progress = progress_units(pipeline, None, 0);
        if progress > 0 {
            return Ok(progress);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    anyhow::bail!("recording made no progress")
}

fn shared_output() -> Arc<Mutex<SharedOutput>> {
    Arc::new(Mutex::new(SharedOutput {
        snapshot: OutputSnapshot {
            desired: OutputState::Running,
            actual: OutputState::Running,
            last_media_at_ns: None,
            error: None,
        },
        recordings: Vec::new(),
    }))
}
