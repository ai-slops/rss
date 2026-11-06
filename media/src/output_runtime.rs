use std::time::Instant;

use gst::prelude::*;
use gstreamer as gst;

use crate::protocol::EngineClockMapping;
use crate::{
    output::{OutputDefinition, OutputError},
    output_pipeline::rtmps_pipeline,
    output_publisher::Publisher,
    output_recording_start::start_recording,
    output_session::RecordingSession,
    recording_clock::RecordingClock,
    source_feeder::SourceFeeders,
};

pub struct ActivePipeline {
    pub(crate) pipeline: gst::Pipeline,
    pub(crate) publisher: Option<Publisher>,
    pub(crate) last_progress_units: u64,
    pub(crate) last_progress: Instant,
    pub(crate) recording_session: Option<RecordingSession>,
    pub(crate) recording_clock: Option<RecordingClock>,
    pub(crate) source_feeders: Option<SourceFeeders>,
}

pub enum PipelinePoll {
    NoProgress,
    Progress,
    Unhealthy(String),
}

impl ActivePipeline {
    pub fn start(
        definition: &OutputDefinition,
        engine_clock: Option<&EngineClockMapping>,
    ) -> Result<Self, OutputError> {
        match definition {
            OutputDefinition::Recording(recording) => start_recording(recording, engine_clock),
            OutputDefinition::Rtmps {
                tap, destination, ..
            } => {
                let publisher = Publisher::spawn(destination)?;
                let pipeline = rtmps_pipeline(tap, publisher.input_fd()?)?;
                pipeline
                    .set_state(gst::State::Playing)
                    .map_err(|_| OutputError::Pipeline)?;
                Ok(Self {
                    pipeline,
                    publisher: Some(publisher),
                    last_progress_units: 0,
                    last_progress: Instant::now(),
                    recording_session: None,
                    recording_clock: None,
                    source_feeders: None,
                })
            }
        }
    }

    pub(crate) fn persist_provenance(&mut self) -> Result<(), OutputError> {
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

#[cfg(test)]
#[path = "output_runtime_lifecycle_tests.rs"]
mod lifecycle_tests;

#[cfg(test)]
#[path = "output_runtime_failure_tests.rs"]
mod failure_tests;
