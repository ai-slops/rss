use std::{
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use gst::prelude::*;
use gstreamer as gst;

use crate::{
    engine_owner::OwnerState,
    engine_source_progress::TimingControl,
    protocol::{EventKind, Milliseconds, Nanoseconds, TrackCorrection},
    timing::{TimingLimits, TimingPlan},
};

const MAXIMUM_BUFFER_MS: i64 = 4_000;

#[derive(Clone)]
struct TimingChange {
    element: Option<gst::Element>,
    control: Option<std::sync::Arc<TimingControl>>,
    offset: i64,
}

#[derive(Clone, Copy)]
enum TimingPhase {
    Armed,
    Applied(Nanoseconds),
    Cancelled,
}

impl TimingPhase {
    fn finish(&mut self) -> Result<Nanoseconds, String> {
        match *self {
            Self::Applied(pts) => Ok(pts),
            Self::Armed | Self::Cancelled => {
                *self = Self::Cancelled;
                Err("timed out waiting for Program timing boundary".to_owned())
            }
        }
    }
}

impl OwnerState {
    pub(super) fn apply_track_timing(
        &self,
        holdback: Milliseconds,
        corrections: &[TrackCorrection],
    ) -> EventKind {
        let changes = match self.timing_changes(holdback, corrections) {
            Ok(changes) => changes,
            Err(event) => return event,
        };
        let Some(compositor) = self.pipeline.by_name("program_compositor") else {
            commit(changes);
            return self.applied(true);
        };
        let Some(output) = compositor.static_pad("src") else {
            return EventKind::Failed {
                reason: "program_compositor has no src pad".to_owned(),
            };
        };
        apply_at_boundary(&output, changes, Duration::from_secs(2)).map_or_else(
            |reason| EventKind::Failed { reason },
            |media_at| self.applied_media(true, media_at),
        )
    }

    fn timing_changes(
        &self,
        holdback: Milliseconds,
        corrections: &[TrackCorrection],
    ) -> Result<Vec<TimingChange>, EventKind> {
        let mut changes = Vec::with_capacity(corrections.len());
        for correction in corrections {
            let Some(element_name) = self.tracks.get(&correction.track_id) else {
                return Err(EventKind::Rejected {
                    reason: format!("unknown track '{}'", correction.track_id.as_str()),
                });
            };
            let plan = TimingPlan::validate(
                &TimingLimits::new(Milliseconds::new(MAXIMUM_BUFFER_MS)),
                holdback,
                correction.correction,
                correction.correction,
            )
            .map_err(|error| EventKind::Rejected {
                reason: error.to_string(),
            })?;
            let element = self.pipeline.by_name(element_name);
            let control = self.timing_controls.get(&correction.track_id).cloned();
            if element.is_none() && control.is_none() {
                return Err(EventKind::Failed {
                    reason: format!("pipeline has no timing element '{element_name}'"),
                });
            }
            let offset = plan.video_delay().value().saturating_mul(1_000_000);
            if let Some(control) = &control {
                let current = control.offset();
                if offset < current {
                    let available = self
                        .track_history
                        .get(&correction.track_id)
                        .and_then(|name| self.pipeline.by_name(name))
                        .map_or(0, |queue| queue.property::<u64>("current-level-time"));
                    let needed = current.saturating_sub(offset).unsigned_abs();
                    if available < needed {
                        return Err(EventKind::Rejected {
                            reason: format!(
                                "track '{}' needs {} ms more retained history",
                                correction.track_id.as_str(),
                                (needed - available) / 1_000_000
                            ),
                        });
                    }
                }
            }
            changes.push(TimingChange {
                element,
                control,
                offset,
            });
        }
        Ok(changes)
    }
}

fn apply_at_boundary(
    output: &gst::Pad,
    changes: Vec<TimingChange>,
    timeout: Duration,
) -> Result<Nanoseconds, String> {
    let phase = Arc::new(Mutex::new(TimingPhase::Armed));
    let callback_phase = Arc::clone(&phase);
    let (sender, receiver) = mpsc::sync_channel(1);
    let probe = output
        .add_probe(
            gst::PadProbeType::BLOCK_DOWNSTREAM | gst::PadProbeType::BUFFER,
            move |_, info| {
                let Some(pts) = info.buffer().and_then(|buffer| buffer.pts()) else {
                    return gst::PadProbeReturn::Pass;
                };
                let Ok(mut phase) = callback_phase.lock() else {
                    return gst::PadProbeReturn::Drop;
                };
                match *phase {
                    TimingPhase::Armed => {
                        commit(changes.clone());
                        *phase = TimingPhase::Applied(Nanoseconds::new(pts.nseconds()));
                        let _ = sender.try_send(());
                    }
                    TimingPhase::Applied(_) | TimingPhase::Cancelled => {}
                }
                gst::PadProbeReturn::Pass
            },
        )
        .ok_or_else(|| "failed to block Program for timing transaction".to_owned())?;
    let _ = receiver.recv_timeout(timeout);
    let result = phase
        .lock()
        .map_err(|_| "Program timing transaction failed".to_owned())
        .and_then(|mut phase| phase.finish());
    output.remove_probe(probe);
    result
}

fn commit(changes: Vec<TimingChange>) {
    for change in changes {
        if let Some(control) = change.control {
            control.set_offset(change.offset);
        } else if let Some(element) = change.element {
            let property = if element.find_property("ts-offset").is_some() {
                "ts-offset"
            } else {
                "timestamp-offset"
            };
            element.set_property(property, change.offset);
        }
    }
}

#[cfg(test)]
#[path = "engine_timing_apply_tests.rs"]
mod tests;
