use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use gst::prelude::*;
use gstreamer as gst;

use crate::{
    output::{
        MediaTap, OutputDefinition, OutputId, OutputSnapshot, OutputState, RecordingDefinition,
        RecordingRecovery,
    },
    output_owner::SharedOutput,
    output_runtime::ActivePipeline,
    output_session::{self, SessionState},
    protocol::{EngineClockMapping, EngineRunId, Nanoseconds, TrackId},
    recording_provenance::RecordingTrackKind,
    source_capture::SourceCaptureRegistry,
    source_stamp::SourceStamp,
    tap::TapChannels,
};

#[test]
fn failed_empty_source_pipeline_teardown_is_bounded() -> anyhow::Result<()> {
    exercise_failed_teardown(false)
}

#[test]
fn failed_video_only_source_pipeline_teardown_is_bounded() -> anyhow::Result<()> {
    exercise_failed_teardown(true)
}

fn exercise_failed_teardown(video: bool) -> anyhow::Result<()> {
    // Given
    let fixture = Fixture::new()?;
    let active = ActivePipeline::start(
        &OutputDefinition::Recording(fixture.definition.clone()),
        Some(&fixture.clock),
    )?;
    let pipeline = active.pipeline.clone();
    let flush_observed = observe_flush_start(&pipeline)?;
    if video {
        fixture.publish_video()?;
        wait_for_provenance(&active)?;
    }
    fixture.registry.close(&fixture.channels);
    let shared = shared_output();

    // When
    let started = Instant::now();
    active.fail(&shared);

    // Then
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(pipeline.state(gst::ClockTime::ZERO).1, gst::State::Null);
    assert!(flush_observed.load(Ordering::Acquire));
    assert_eq!(fixture.registry.subscriber_count(&fixture.channels), 0);
    assert_failed_session(fixture.directory.path(), video)?;
    assert!(
        shared.lock().expect("shared output").recordings.iter().all(
            |recording| !recording.decoded && recording.recovery != RecordingRecovery::Complete
        )
    );
    Ok(())
}

fn observe_flush_start(pipeline: &gst::Pipeline) -> anyhow::Result<Arc<AtomicBool>> {
    let observed = Arc::new(AtomicBool::new(false));
    let probe_observed = Arc::clone(&observed);
    pipeline
        .by_name("recording_video_input")
        .and_then(|source| source.static_pad("src"))
        .ok_or_else(|| anyhow::anyhow!("recording video appsrc"))?
        .add_probe(gst::PadProbeType::EVENT_FLUSH, move |_, info| {
            if info
                .event()
                .is_some_and(|event| matches!(event.view(), gst::EventView::FlushStart(_)))
            {
                probe_observed.store(true, Ordering::Release);
            }
            gst::PadProbeReturn::Ok
        })
        .ok_or_else(|| anyhow::anyhow!("flush probe"))?;
    Ok(observed)
}

struct Fixture {
    directory: tempfile::TempDir,
    registry: Arc<SourceCaptureRegistry>,
    channels: TapChannels,
    definition: RecordingDefinition,
    clock: EngineClockMapping,
}

impl Fixture {
    fn new() -> anyhow::Result<Self> {
        gst::init()?;
        let directory = tempfile::tempdir()?;
        let run = EngineRunId::new("failure-run")?;
        let channels = TapChannels::clean(
            &run,
            &TrackId::new("failure-video")?,
            &TrackId::new("failure-audio")?,
        );
        let registry = Arc::new(SourceCaptureRegistry::default());
        let definition = RecordingDefinition::new(
            OutputId::new("failure-recording")?,
            MediaTap::new(&channels.video, &channels.audio)?,
            directory.path(),
            Duration::from_secs(1),
        )?
        .with_engine_run_id(&run)
        .with_source_capture(registry.capture(&channels));
        Ok(Self {
            directory,
            registry,
            channels,
            definition,
            clock: EngineClockMapping {
                media_time: Nanoseconds::new(1_000_000_000),
                wall_time: Nanoseconds::new(1_000_000_000),
                uncertainty: Nanoseconds::new(0),
            },
        })
    }

    fn publish_video(&self) -> anyhow::Result<()> {
        let caps = gst::Caps::builder("video/x-raw")
            .field("format", "I420")
            .field("width", 1280_i32)
            .field("height", 720_i32)
            .field("framerate", gst::Fraction::new(30, 1))
            .build();
        self.registry.publish_for_test(
            &self.channels.video,
            gst::Buffer::with_size(1280 * 720 * 3 / 2)?,
            caps,
            SourceStamp {
                engine_run_id: "failure-run".to_owned(),
                source_id: "failure-source".to_owned(),
                track_id: "failure-video".to_owned(),
                track_kind: RecordingTrackKind::Video,
                continuity_generation: 1,
                media_pts_ns: 1_100_000_000,
                duration_ns: 33_333_333,
            },
        );
        Ok(())
    }
}

fn wait_for_provenance(active: &ActivePipeline) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if active
            .recording_clock
            .as_ref()
            .and_then(crate::recording_clock::RecordingClock::provenance)
            .is_some_and(|snapshot| !snapshot.intervals.is_empty())
        {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    anyhow::bail!("video provenance was not observed")
}

fn assert_failed_session(root: &Path, provenance: bool) -> anyhow::Result<()> {
    let directory = std::fs::read_dir(root)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.join("session.json").is_file())
        .ok_or_else(|| anyhow::anyhow!("session directory"))?;
    let metadata = output_session::read(&directory).metadata;
    assert_eq!(metadata.state, SessionState::Failed);
    assert_eq!(!metadata.provenance.is_empty(), provenance);
    assert!(directory.join("catalog.json").is_file());
    Ok(())
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
