use gst::prelude::*;
use gstreamer as gst;

use crate::{
    engine_owner::OwnerState,
    protocol::{EventKind, Milliseconds},
    timing::{TimingLimits, TimingPlan},
};

const MAXIMUM_BUFFER_MS: i64 = 5_000;

impl OwnerState {
    pub(super) fn apply_timing(
        &self,
        timing: (Milliseconds, Milliseconds, Milliseconds),
    ) -> EventKind {
        let (holdback, video_correction, audio_correction) = timing;
        let plan = TimingPlan::validate(
            &TimingLimits::new(Milliseconds::new(MAXIMUM_BUFFER_MS)),
            holdback,
            video_correction,
            audio_correction,
        );
        let Ok(plan) = plan else {
            return EventKind::Rejected {
                reason: plan
                    .err()
                    .map_or_else(|| "invalid timing".to_owned(), |error| error.to_string()),
            };
        };
        let Some(video_timing) = self.pipeline.by_name("video_timing_source") else {
            return EventKind::Failed {
                reason: "pipeline has no video_timing_source element".to_owned(),
            };
        };
        let Some(audio_timing) = self.pipeline.by_name("audio_timing_source") else {
            return EventKind::Failed {
                reason: "pipeline has no audio_timing_source element".to_owned(),
            };
        };
        video_timing.set_property("timestamp-offset", to_nanoseconds(plan.video_delay()));
        audio_timing.set_property("timestamp-offset", to_nanoseconds(plan.audio_delay()));
        self.applied(true)
    }
}

const fn to_nanoseconds(value: Milliseconds) -> i64 {
    value.value().saturating_mul(1_000_000)
}
