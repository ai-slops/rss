use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use gst::prelude::*;
use gstreamer as gst;
use uuid::Uuid;

use crate::{
    output::{OutputDefinition, OutputError},
    output_catalog,
    output_owner::SharedOutput,
    output_pipeline::{recording_pipeline, rtmps_pipeline},
    output_publisher::Publisher,
};

const STALL_TIMEOUT: Duration = Duration::from_secs(5);

pub struct ActivePipeline {
    pipeline: gst::Pipeline,
    publisher: Option<Publisher>,
    started_at_ns: u64,
    last_progress_units: u64,
    last_progress: Instant,
    recording_directory: Option<PathBuf>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PipelinePoll {
    NoProgress,
    Progress,
    Unhealthy,
}

impl ActivePipeline {
    pub fn start(definition: &OutputDefinition) -> Result<Self, OutputError> {
        let (pipeline, publisher, recording_directory) = match definition {
            OutputDefinition::Recording(definition) => {
                let session = definition.directory.join(Uuid::now_v7().to_string());
                fs::create_dir_all(&session).map_err(|_| OutputError::Pipeline)?;
                let pattern = session.join("segment-%05d.mkv");
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
        pipeline
            .set_state(gst::State::Playing)
            .map_err(|_| OutputError::Pipeline)?;
        Ok(Self {
            pipeline,
            publisher,
            started_at_ns: now_ns(),
            last_progress_units: 0,
            last_progress: Instant::now(),
            recording_directory,
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
        self.catalog(shared)?;
        finalized.then_some(()).ok_or(OutputError::Pipeline)
    }

    pub fn fail(mut self, shared: &Arc<Mutex<SharedOutput>>) {
        let _ = self.pipeline.set_state(gst::State::Null);
        if let Some(publisher) = self.publisher.as_mut() {
            publisher.stop(true);
        }
        let _ = self.recover_catalog(shared);
    }

    pub fn poll(&mut self, shared: &Arc<Mutex<SharedOutput>>) -> PipelinePoll {
        if self.publisher.as_mut().is_some_and(Publisher::has_exited) {
            return PipelinePoll::Unhealthy;
        }
        if let Some(bus) = self.pipeline.bus()
            && bus
                .timed_pop_filtered(gst::ClockTime::ZERO, &[gst::MessageType::Error])
                .is_some()
        {
            return PipelinePoll::Unhealthy;
        }
        let progress = self.progress_units();
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

    fn progress_units(&self) -> u64 {
        self.publisher.as_ref().map_or_else(
            || {
                self.pipeline
                    .by_name("output_activity")
                    .and_then(|identity| {
                        identity
                            .property::<gst::Structure>("stats")
                            .get::<u64>("num-buffers")
                            .ok()
                    })
                    .unwrap_or(self.last_progress_units)
            },
            Publisher::progress_bytes,
        )
    }

    fn catalog(&mut self, shared: &Arc<Mutex<SharedOutput>>) -> Result<(), OutputError> {
        let Some(directory) = self.recording_directory.take() else {
            return Ok(());
        };
        let recordings = output_catalog::finalize(&directory, self.started_at_ns, now_ns())?;
        if let Ok(mut state) = shared.lock() {
            state.recordings.extend(recordings);
        }
        Ok(())
    }

    fn recover_catalog(&mut self, shared: &Arc<Mutex<SharedOutput>>) -> Result<(), OutputError> {
        let Some(directory) = self.recording_directory.take() else {
            return Ok(());
        };
        let recordings = output_catalog::recover(&directory, self.started_at_ns, now_ns())?;
        if let Ok(mut state) = shared.lock() {
            state.recordings.extend(recordings);
        }
        Ok(())
    }
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            duration.as_nanos().try_into().unwrap_or(u64::MAX)
        })
}
