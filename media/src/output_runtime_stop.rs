use std::sync::{Arc, Mutex};

use gst::prelude::*;
use gstreamer as gst;

use crate::{
    output::{OutputDefinition, OutputError},
    output_catalog,
    output_owner::SharedOutput,
    output_runtime::ActivePipeline,
    output_runtime_support::{now_ns, replace_recordings},
    output_session::SessionState,
    recording_clock::RecordingClock,
};

impl ActivePipeline {
    pub fn stop(
        mut self,
        definition: &OutputDefinition,
        shared: &Arc<Mutex<SharedOutput>>,
    ) -> Result<(), OutputError> {
        if matches!(definition, OutputDefinition::Rtmps { .. }) {
            let result = self
                .pipeline
                .set_state(gst::State::Null)
                .map_err(|_| OutputError::Pipeline);
            if let Some(publisher) = self.publisher.as_mut() {
                publisher.stop(true);
            }
            return result.map(|_| ());
        }
        self.stop_recording(shared)
    }

    pub fn fail(mut self, shared: &Arc<Mutex<SharedOutput>>) {
        flush_failed_pipeline(&self.pipeline);
        if let Some(feeders) = self.source_feeders.take() {
            let _ = feeders.stop();
        }
        let _ = self.pipeline.set_state(gst::State::Null);
        if let Some(publisher) = self.publisher.as_mut() {
            publisher.stop(true);
        }
        let _ = self.finish_catalog(shared, SessionState::Failed, true);
    }

    fn stop_recording(&mut self, shared: &Arc<Mutex<SharedOutput>>) -> Result<(), OutputError> {
        let mut failure = None;
        let pending_feeder_error = self
            .source_feeders
            .as_ref()
            .and_then(crate::source_feeder::SourceFeeders::error);
        if let Some(error) = pending_feeder_error {
            first_error(&mut failure, OutputError::PipelineDetail(error));
            flush_failed_pipeline(&self.pipeline);
        }
        if let Some(feeders) = self.source_feeders.take()
            && let Err(error) = feeders.stop()
        {
            first_error(&mut failure, OutputError::PipelineDetail(error));
            flush_failed_pipeline(&self.pipeline);
        }
        if failure.is_none() && self.last_progress_units > 0 {
            let bus = self.pipeline.bus();
            if bus.is_none() {
                first_error(
                    &mut failure,
                    OutputError::PipelineDetail("recording pipeline has no bus".to_owned()),
                );
            }
            if !self.pipeline.send_event(gst::event::Eos::new()) {
                first_error(
                    &mut failure,
                    OutputError::PipelineDetail("recording pipeline rejected EOS".to_owned()),
                );
            } else if let Some(bus) = bus {
                let message = bus.timed_pop_filtered(
                    gst::ClockTime::from_seconds(15),
                    &[gst::MessageType::Eos, gst::MessageType::Error],
                );
                if !message
                    .as_ref()
                    .is_some_and(|value| matches!(value.view(), gst::MessageView::Eos(_)))
                {
                    first_error(
                        &mut failure,
                        OutputError::PipelineDetail(
                            "recording pipeline did not finalize with EOS".to_owned(),
                        ),
                    );
                }
            }
        }
        if self.pipeline.set_state(gst::State::Null).is_err() {
            first_error(
                &mut failure,
                OutputError::PipelineDetail("recording pipeline failed to enter Null".to_owned()),
            );
        }
        let complete = failure.is_none();
        if let Err(error) = self.finish_catalog(
            shared,
            if complete {
                SessionState::Complete
            } else {
                SessionState::Failed
            },
            !complete,
        ) {
            first_error(&mut failure, error);
        }
        failure.map_or(Ok(()), Err)
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
        let mut failure = None;
        if let Some(provenance) = self
            .recording_clock
            .as_ref()
            .and_then(RecordingClock::provenance)
            && let Err(error) = session.update_provenance(provenance)
        {
            first_error(&mut failure, error);
        }
        if let Err(error) = session.finish(state, now_ns()) {
            first_error(&mut failure, error);
        }
        let recordings = if recover {
            output_catalog::recover(&session, self.recording_clock.as_ref())
        } else {
            output_catalog::finalize(&session, self.recording_clock.as_ref())
        };
        match recordings {
            Ok(recordings) => {
                replace_recordings(shared, &session.metadata.session_id, recordings);
            }
            Err(error) => first_error(&mut failure, error),
        }
        failure.map_or(Ok(()), Err)
    }
}

fn flush_failed_pipeline(pipeline: &gst::Pipeline) {
    let _ = pipeline.send_event(gst::event::FlushStart::new());
}

fn first_error(slot: &mut Option<OutputError>, error: OutputError) {
    if slot.is_none() {
        *slot = Some(error);
    }
}
