use std::{
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    output::{OutputDefinition, RecordingSegment},
    output_owner::SharedOutput,
    output_publisher::Publisher,
};
use gstreamer::{self as gst, prelude::*};

pub fn progress_units(
    pipeline: &gst::Pipeline,
    publisher: Option<&Publisher>,
    previous: u64,
) -> u64 {
    publisher.map_or_else(
        || {
            pipeline
                .by_name("output_activity")
                .and_then(|identity| {
                    identity
                        .property::<gst::Structure>("stats")
                        .get::<u64>("num-buffers")
                        .ok()
                })
                .unwrap_or(previous)
        },
        Publisher::progress_bytes,
    )
}

pub fn replace_recordings(
    shared: &Arc<Mutex<SharedOutput>>,
    session_id: &str,
    recordings: Vec<RecordingSegment>,
) {
    if let Ok(mut state) = shared.lock() {
        state
            .recordings
            .retain(|recording| recording.session_id != session_id);
        state.recordings.extend(recordings);
    }
}

pub fn clock_engine_run_id(definition: &OutputDefinition) -> Option<String> {
    match definition {
        OutputDefinition::Recording(recording) => recording.engine_run_id.clone(),
        OutputDefinition::Rtmps { .. } => None,
    }
}

pub const fn capture_source_provenance(definition: &OutputDefinition) -> bool {
    match definition {
        OutputDefinition::Recording(recording) => recording.capture_source_provenance,
        OutputDefinition::Rtmps { .. } => false,
    }
}

pub fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            duration.as_nanos().try_into().unwrap_or(u64::MAX)
        })
}
