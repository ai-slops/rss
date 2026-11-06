use std::{sync::Arc, time::Instant};

use gst::prelude::*;
use gstreamer as gst;

use crate::{
    output::{OutputError, RecordingDefinition},
    output_catalog,
    output_pipeline::{recording_pipeline, source_recording_pipeline},
    output_runtime::ActivePipeline,
    output_runtime_support::now_ns,
    output_session::{RecordingSession, SessionStart, SessionState},
    protocol::EngineClockMapping,
    recording_clock::RecordingClock,
    recording_provenance::ProvenanceCollector,
    source_capture::SourceCapture,
    source_feeder::SourceFeeders,
};

pub struct FeederStart<'a> {
    pub pipeline: &'a gst::Pipeline,
    pub capture: &'a SourceCapture,
    pub media_origin_ns: u64,
    pub provenance: &'a Arc<ProvenanceCollector>,
}

pub struct RecordingStartHooks<P, F> {
    pub build_pipeline: P,
    pub start_feeders: F,
}

pub fn start_recording(
    definition: &RecordingDefinition,
    engine_clock: Option<&EngineClockMapping>,
) -> Result<ActivePipeline, OutputError> {
    start_recording_with(
        definition,
        engine_clock,
        RecordingStartHooks {
            build_pipeline: |recording: &RecordingDefinition, pattern: &str| {
                if recording.source_capture.is_some() {
                    source_recording_pipeline(pattern, recording.segment_duration)
                } else {
                    recording_pipeline(&recording.tap, pattern, recording.segment_duration)
                }
            },
            start_feeders: |request: FeederStart<'_>| {
                SourceFeeders::start(
                    request.pipeline,
                    request.capture,
                    request.media_origin_ns,
                    request.provenance,
                )
            },
        },
    )
}

pub fn start_recording_with<P, F>(
    definition: &RecordingDefinition,
    engine_clock: Option<&EngineClockMapping>,
    hooks: RecordingStartHooks<P, F>,
) -> Result<ActivePipeline, OutputError>
where
    P: FnOnce(&RecordingDefinition, &str) -> Result<gst::Pipeline, OutputError>,
    F: FnOnce(FeederStart<'_>) -> Result<SourceFeeders, String>,
{
    let session = RecordingSession::begin(SessionStart {
        root: &definition.directory,
        engine_run_id: definition.engine_run_id.clone(),
        clock_anchor: engine_clock.cloned(),
        wall_start_ns: now_ns(),
    })?;
    let mut startup = RecordingStartup::new(session);
    let pattern = startup
        .session
        .as_ref()
        .and_then(|value| value.pattern().to_str().map(str::to_owned))
        .ok_or_else(|| OutputError::PipelineDetail("recording path is not UTF-8".to_owned()))?;
    startup.pipeline = Some((hooks.build_pipeline)(definition, &pattern)?);
    let pipeline = startup.pipeline.as_ref().ok_or(OutputError::Pipeline)?;
    pipeline.set_state(gst::State::Playing).map_err(|_| {
        OutputError::PipelineDetail("recording pipeline failed to enter Playing".to_owned())
    })?;
    if let Some(capture) = definition.source_capture.as_ref() {
        let anchor = engine_clock.ok_or_else(|| {
            OutputError::PipelineDetail("source recording requires an engine clock".to_owned())
        })?;
        let provenance = Arc::new(ProvenanceCollector::default());
        let mut clock = RecordingClock::source_capture(
            anchor,
            definition.engine_run_id.clone(),
            Arc::clone(&provenance),
        );
        clock.set_media_origin(capture.recording_origin(clock.media_origin_ns));
        startup.feeders = Some(
            (hooks.start_feeders)(FeederStart {
                pipeline,
                capture,
                media_origin_ns: clock.media_origin_ns,
                provenance: &provenance,
            })
            .map_err(OutputError::PipelineDetail)?,
        );
        startup.clock = Some(clock);
    } else if let Some(anchor) = engine_clock {
        startup.clock = RecordingClock::sample(
            pipeline,
            anchor,
            definition.engine_run_id.clone(),
            definition.capture_source_provenance,
        );
    }
    startup.commit()
}

struct RecordingStartup {
    pipeline: Option<gst::Pipeline>,
    session: Option<RecordingSession>,
    clock: Option<RecordingClock>,
    feeders: Option<SourceFeeders>,
    committed: bool,
}

impl RecordingStartup {
    const fn new(session: RecordingSession) -> Self {
        Self {
            pipeline: None,
            session: Some(session),
            clock: None,
            feeders: None,
            committed: false,
        }
    }

    fn commit(mut self) -> Result<ActivePipeline, OutputError> {
        let pipeline = self.pipeline.take().ok_or(OutputError::Pipeline)?;
        self.committed = true;
        Ok(ActivePipeline {
            pipeline,
            publisher: None,
            last_progress_units: 0,
            last_progress: Instant::now(),
            recording_session: self.session.take(),
            recording_clock: self.clock.take(),
            source_feeders: self.feeders.take(),
        })
    }

    fn rollback(&mut self) {
        if let Some(pipeline) = self.pipeline.as_ref() {
            let _ = pipeline.send_event(gst::event::FlushStart::new());
        }
        if let Some(feeders) = self.feeders.take() {
            let _ = feeders.stop();
        }
        if let Some(pipeline) = self.pipeline.as_ref() {
            let _ = pipeline.set_state(gst::State::Null);
            let _ = pipeline.state(gst::ClockTime::from_seconds(5));
        }
        let Some(session) = self.session.as_mut() else {
            return;
        };
        if let Some(provenance) = self.clock.as_ref().and_then(RecordingClock::provenance) {
            let _ = session.update_provenance(provenance);
        }
        let _ = session.finish(SessionState::Failed, now_ns());
        let _ = output_catalog::recover(session, self.clock.as_ref());
    }
}

impl Drop for RecordingStartup {
    fn drop(&mut self) {
        if !self.committed {
            self.rollback();
        }
    }
}
