use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use gst::prelude::*;
use gstreamer as gst;

use crate::protocol::EngineClockMapping;
use crate::{
    output::{OutputDefinition, OutputError},
    output_catalog,
    output_owner::SharedOutput,
    output_pipeline::{recording_pipeline, rtmps_pipeline},
    output_publisher::Publisher,
    output_runtime_support::{
        capture_source_provenance, clock_engine_run_id, now_ns, progress_units, replace_recordings,
    },
    output_session::{RecordingSession, SessionStart, SessionState},
    recording_clock::RecordingClock,
};

const STALL_TIMEOUT: Duration = Duration::from_secs(5);

pub struct ActivePipeline {
    pipeline: gst::Pipeline,
    publisher: Option<Publisher>,
    last_progress_units: u64,
    last_progress: Instant,
    recording_session: Option<RecordingSession>,
    recording_clock: Option<RecordingClock>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PipelinePoll {
    NoProgress,
    Progress,
    Unhealthy,
}

impl ActivePipeline {
    pub fn start(
        definition: &OutputDefinition,
        engine_clock: Option<&EngineClockMapping>,
    ) -> Result<Self, OutputError> {
        let started_at_ns = now_ns();
        let (pipeline, publisher, mut recording_session) = match definition {
            OutputDefinition::Recording(definition) => {
                let session = RecordingSession::begin(SessionStart {
                    root: &definition.directory,
                    engine_run_id: definition.engine_run_id.clone(),
                    clock_anchor: engine_clock.cloned(),
                    wall_start_ns: started_at_ns,
                })?;
                let pattern = session.pattern();
                let pattern = pattern.to_str().ok_or(OutputError::Pipeline)?;
                (
                    recording_pipeline(&definition.tap, pattern, definition.segment_duration)?,
                    None,
                    Some(session),
                )
            }
            OutputDefinition::Rtmps {
                tap, destination, ..
            } => {
                let publisher = Publisher::spawn(destination)?;
                let pipeline = rtmps_pipeline(tap, publisher.input_fd()?)?;
                (pipeline, Some(publisher), None)
            }
        };
        if pipeline.set_state(gst::State::Playing).is_err() {
            let _ = pipeline.set_state(gst::State::Null);
            if let Some(session) = recording_session.as_mut() {
                let _ = session.finish(SessionState::Failed, now_ns());
            }
            return Err(OutputError::Pipeline);
        }
        let recording_clock = engine_clock.and_then(|clock| {
            RecordingClock::sample(
                &pipeline,
                clock,
                clock_engine_run_id(definition),
                capture_source_provenance(definition),
            )
        });
        Ok(Self {
            pipeline,
            publisher,
            last_progress_units: 0,
            last_progress: Instant::now(),
            recording_session,
            recording_clock,
        })
    }

    pub fn stop(
        mut self,
        definition: &OutputDefinition,
        shared: &Arc<Mutex<SharedOutput>>,
    ) -> Result<(), OutputError> {
        if matches!(definition, OutputDefinition::Rtmps { .. }) {
            self.pipeline
                .set_state(gst::State::Null)
                .map_err(|_| OutputError::Pipeline)?;
            if let Some(publisher) = self.publisher.as_mut() {
                publisher.stop(true);
            }
            return Ok(());
        }
        if self.last_progress_units == 0 {
            self.pipeline
                .set_state(gst::State::Null)
                .map_err(|_| OutputError::Pipeline)?;
            if let Some(session) = self.recording_session.as_mut() {
                session.finish(SessionState::Complete, now_ns())?;
                let recordings = output_catalog::finalize(session, self.recording_clock.as_ref())?;
                replace_recordings(shared, &session.metadata.session_id, recordings);
            }
            return Ok(());
        }
        let bus = self.pipeline.bus().ok_or(OutputError::Pipeline)?;
        if !self.pipeline.send_event(gst::event::Eos::new()) {
            let _ = self.pipeline.set_state(gst::State::Null);
            return Err(OutputError::Pipeline);
        }
        let message = bus.timed_pop_filtered(
            gst::ClockTime::from_seconds(15),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        );
        let finalized = message
            .as_ref()
            .is_some_and(|message| matches!(message.view(), gst::MessageView::Eos(_)));
        self.pipeline
            .set_state(gst::State::Null)
            .map_err(|_| OutputError::Pipeline)?;
        if finalized {
            self.finish_catalog(shared, SessionState::Complete, false)?;
            Ok(())
        } else {
            self.finish_catalog(shared, SessionState::Failed, true)?;
            Err(OutputError::Pipeline)
        }
    }

    pub fn fail(mut self, shared: &Arc<Mutex<SharedOutput>>) {
        let _ = self.pipeline.set_state(gst::State::Null);
        if let Some(publisher) = self.publisher.as_mut() {
            publisher.stop(true);
        }
        let _ = self.finish_catalog(shared, SessionState::Failed, true);
    }

    pub fn poll(&mut self, shared: &Arc<Mutex<SharedOutput>>) -> PipelinePoll {
        if self.publisher.as_mut().is_some_and(Publisher::has_exited) {
            return PipelinePoll::Unhealthy;
        }
        if self.poll_bus(shared) {
            return PipelinePoll::Unhealthy;
        }
        let progress = progress_units(
            &self.pipeline,
            self.publisher.as_ref(),
            self.last_progress_units,
        );
        if progress > self.last_progress_units {
            self.last_progress_units = progress;
            self.last_progress = Instant::now();
            if let Ok(mut state) = shared.lock() {
                state.snapshot.last_media_at_ns = Some(now_ns());
            }
            return PipelinePoll::Progress;
        }
        if self.last_progress.elapsed() >= STALL_TIMEOUT {
            PipelinePoll::Unhealthy
        } else {
            PipelinePoll::NoProgress
        }
    }

    fn poll_bus(&mut self, shared: &Arc<Mutex<SharedOutput>>) -> bool {
        let Some(bus) = self.pipeline.bus() else {
            return true;
        };
        while let Some(message) = bus.pop() {
            match message.view() {
                gst::MessageView::Error(_) => return true,
                gst::MessageView::Element(element) => {
                    let Some(structure) = element.structure() else {
                        continue;
                    };
                    if structure.name() != "splitmuxsink-fragment-closed" {
                        continue;
                    }
                    let Ok(location) = structure.get::<String>("location") else {
                        continue;
                    };
                    if self.persist_provenance().is_err() {
                        continue;
                    }
                    if let Some(session) = self.recording_session.as_ref()
                        && let Ok(recordings) = output_catalog::publish_fragment(
                            session,
                            std::path::Path::new(&location),
                            self.recording_clock.as_ref(),
                        )
                    {
                        replace_recordings(shared, &session.metadata.session_id, recordings);
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn finish_catalog(
        &mut self,
        shared: &Arc<Mutex<SharedOutput>>,
        state: SessionState,
        recover: bool,
    ) -> Result<(), OutputError> {
        let Some(mut session) = self.recording_session.take() else {
            return Ok(());
        };
        if let Some(provenance) = self
            .recording_clock
            .as_ref()
            .and_then(RecordingClock::provenance)
        {
            session.update_provenance(provenance)?;
        }
        let metadata_result = session.finish(state, now_ns());
        let recordings = if recover {
            output_catalog::recover(&session, self.recording_clock.as_ref())?
        } else {
            output_catalog::finalize(&session, self.recording_clock.as_ref())?
        };
        replace_recordings(shared, &session.metadata.session_id, recordings);
        metadata_result
    }

    fn persist_provenance(&mut self) -> Result<(), OutputError> {
        let Some(provenance) = self
            .recording_clock
            .as_ref()
            .and_then(RecordingClock::provenance)
        else {
            return Ok(());
        };
        self.recording_session
            .as_mut()
            .map_or(Ok(()), |session| session.update_provenance(provenance))
    }
}
