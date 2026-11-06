use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use super::*;
use crate::{
    output::{MediaTap, OutputId, OutputSnapshot, OutputState, RecordingDefinition},
    output_owner::SharedOutput,
    output_pipeline::source_recording_pipeline,
    output_recording_start::{FeederStart, RecordingStartHooks, start_recording_with},
    output_session::{self, SessionState},
    protocol::{EngineRunId, Nanoseconds, TrackId},
    recording_provenance::RecordingTrackKind,
    source_capture::SourceCaptureRegistry,
    source_stamp::SourceStamp,
    tap::TapChannels,
};

#[test]
fn missing_clock_rolls_pipeline_to_null_and_marks_session_failed() -> anyhow::Result<()> {
    // Given
    let fixture = Fixture::new()?;
    let observed = Arc::new(Mutex::new(None));

    // When
    let result = start_recording_with(
        &fixture.definition,
        None,
        hooks(Arc::clone(&observed), None),
    );

    // Then
    assert!(result.is_err());
    assert_null(&observed)?;
    assert_failed_session(fixture.directory.path(), false)?;
    Ok(())
}

#[test]
fn feeder_start_failure_rolls_pipeline_to_null_and_preserves_detail() -> anyhow::Result<()> {
    // Given
    let fixture = Fixture::new()?;
    let observed = Arc::new(Mutex::new(None));

    // When
    let result = start_recording_with(
        &fixture.definition,
        Some(&fixture.clock),
        hooks(
            Arc::clone(&observed),
            Some("injected feeder startup failure"),
        ),
    );

    // Then
    let error = match result {
        Ok(_) => anyhow::bail!("startup should fail"),
        Err(error) => error.to_string(),
    };
    assert!(error.contains("injected feeder startup failure"), "{error}");
    assert_null(&observed)?;
    assert_failed_session(fixture.directory.path(), false)?;
    Ok(())
}

#[test]
fn sticky_feeder_fault_before_stop_still_nulls_and_recovers_provenance() -> anyhow::Result<()> {
    // Given
    let fixture = Fixture::new()?;
    let mut active = ActivePipeline::start(
        &OutputDefinition::Recording(fixture.definition.clone()),
        Some(&fixture.clock),
    )?;
    let pipeline = active.pipeline.clone();
    fixture.publish_video()?;
    fixture.publish_audio()?;
    wait_for_provenance(&active)?;
    active.last_progress_units = 1;
    fixture.registry.close(&fixture.channels);
    let shared = shared_output();

    // When
    let started = Instant::now();
    let result = active.stop(
        &OutputDefinition::Recording(fixture.definition.clone()),
        &shared,
    );

    // Then
    assert!(result.is_err());
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(pipeline.state(gst::ClockTime::ZERO).1, gst::State::Null);
    assert_eq!(fixture.registry.subscriber_count(&fixture.channels), 0);
    assert_failed_session(fixture.directory.path(), true)?;
    Ok(())
}

struct Fixture {
    directory: tempfile::TempDir,
    registry: Arc<SourceCaptureRegistry>,
    channels: TapChannels,
    definition: RecordingDefinition,
    clock: EngineClockMapping,
}

type PipelineHook = Box<dyn Fn(&RecordingDefinition, &str) -> Result<gst::Pipeline, OutputError>>;
type FeederHook = Box<dyn for<'a> Fn(FeederStart<'a>) -> Result<SourceFeeders, String>>;

impl Fixture {
    fn new() -> anyhow::Result<Self> {
        gst::init()?;
        let directory = tempfile::tempdir()?;
        let run = EngineRunId::new("lifecycle-run")?;
        let channels = TapChannels::clean(
            &run,
            &TrackId::new("lifecycle-video")?,
            &TrackId::new("lifecycle-audio")?,
        );
        let registry = Arc::new(SourceCaptureRegistry::default());
        let definition = RecordingDefinition::new(
            OutputId::new("lifecycle-recording")?,
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
        let buffer = gst::Buffer::with_size(1280 * 720 * 3 / 2)?;
        self.registry.publish_for_test(
            &self.channels.video,
            buffer,
            caps,
            SourceStamp {
                engine_run_id: "lifecycle-run".to_owned(),
                source_id: "lifecycle-source".to_owned(),
                track_id: "lifecycle-video".to_owned(),
                track_kind: RecordingTrackKind::Video,
                continuity_generation: 1,
                media_pts_ns: 1_100_000_000,
                duration_ns: 33_333_333,
            },
        );
        Ok(())
    }

    fn publish_audio(&self) -> anyhow::Result<()> {
        let caps = gst::Caps::builder("audio/x-raw")
            .field("format", "S16LE")
            .field("rate", 48_000_i32)
            .field("channels", 2_i32)
            .build();
        let buffer = gst::Buffer::with_size(480 * 2 * 2)?;
        self.registry.publish_for_test(
            &self.channels.audio,
            buffer,
            caps,
            SourceStamp {
                engine_run_id: "lifecycle-run".to_owned(),
                source_id: "lifecycle-source".to_owned(),
                track_id: "lifecycle-audio".to_owned(),
                track_kind: RecordingTrackKind::Audio,
                continuity_generation: 1,
                media_pts_ns: 1_100_000_000,
                duration_ns: 10_000_000,
            },
        );
        Ok(())
    }
}

fn hooks(
    observed: Arc<Mutex<Option<gst::Pipeline>>>,
    feeder_error: Option<&'static str>,
) -> RecordingStartHooks<PipelineHook, FeederHook> {
    RecordingStartHooks {
        build_pipeline: Box::new(move |definition: &RecordingDefinition, pattern: &str| {
            let pipeline = source_recording_pipeline(pattern, definition.segment_duration)?;
            *observed.lock().expect("observed pipeline") = Some(pipeline.clone());
            Ok(pipeline)
        }),
        start_feeders: Box::new(move |request: FeederStart<'_>| {
            feeder_error.map_or_else(
                || {
                    SourceFeeders::start(
                        request.pipeline,
                        request.capture,
                        request.media_origin_ns,
                        request.provenance,
                    )
                },
                |message| Err(message.to_owned()),
            )
        }),
    }
}

fn assert_null(observed: &Mutex<Option<gst::Pipeline>>) -> anyhow::Result<()> {
    let pipeline = observed
        .lock()
        .map_err(|_| anyhow::anyhow!("pipeline lock"))?
        .clone()
        .ok_or_else(|| anyhow::anyhow!("pipeline not built"))?;
    assert_eq!(pipeline.state(gst::ClockTime::ZERO).1, gst::State::Null);
    Ok(())
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

fn wait_for_provenance(active: &ActivePipeline) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if active
            .recording_clock
            .as_ref()
            .and_then(RecordingClock::provenance)
            .is_some_and(|snapshot| !snapshot.intervals.is_empty())
        {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    anyhow::bail!("provenance was not observed")
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
