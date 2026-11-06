use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use gst::prelude::*;
use gstreamer as gst;

use crate::{
    output_catalog,
    output_owner::SharedOutput,
    output_publisher::Publisher,
    output_runtime::{ActivePipeline, PipelinePoll},
    output_runtime_support::{now_ns, progress_units, replace_recordings},
    source_feeder::SourceFeeders,
};

const STALL_TIMEOUT: Duration = Duration::from_secs(5);

impl ActivePipeline {
    pub fn poll(&mut self, shared: &Arc<Mutex<SharedOutput>>) -> PipelinePoll {
        if let Some(error) = self.source_feeders.as_ref().and_then(SourceFeeders::error) {
            return PipelinePoll::Unhealthy(error);
        }
        if self.publisher.as_mut().is_some_and(Publisher::has_exited) {
            return PipelinePoll::Unhealthy("Network publisher process exited.".to_owned());
        }
        if let Some(error) = self.poll_bus(shared) {
            return PipelinePoll::Unhealthy(error);
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
            PipelinePoll::Unhealthy("Output pipeline made no progress for five seconds.".to_owned())
        } else {
            PipelinePoll::NoProgress
        }
    }

    fn poll_bus(&mut self, shared: &Arc<Mutex<SharedOutput>>) -> Option<String> {
        let Some(bus) = self.pipeline.bus() else {
            return Some("Output pipeline has no GStreamer bus.".to_owned());
        };
        while let Some(message) = bus.pop() {
            match message.view() {
                gst::MessageView::Error(error) => {
                    return Some(format!(
                        "GStreamer output pipeline failed: {} ({:?})",
                        error.error(),
                        error.debug()
                    ));
                }
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
        None
    }
}
