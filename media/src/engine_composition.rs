use crate::{
    engine_composition_transform::validate_placements,
    engine_frame_swap::{FrameSwap, SwapResult},
    engine_layer::{CompositionLayerRuntime, build_layer},
    engine_owner::OwnerState,
    engine_pad_values::PadChange,
    protocol::{CompositionPlacement, EventKind, Nanoseconds, ProgramRevision, TrackId},
};
use gstreamer::{self as gst, prelude::*};
use std::time::{Duration, Instant};

pub struct CompositionState {
    target: &'static str,
    current: Vec<CompositionLayerRuntime>,
    pending: Option<PendingCleanup>,
}

struct PendingCleanup {
    fence: FrameSwap,
    _retired: Vec<CompositionLayerRuntime>,
    started: Instant,
    warned: bool,
}

#[derive(Clone, Copy)]
pub struct Layout<'a> {
    pub placements: &'a [CompositionPlacement],
    pub tracks: &'a std::collections::HashMap<TrackId, String>,
}

impl CompositionState {
    pub const fn new(target: &'static str) -> Self {
        Self {
            target,
            current: Vec::new(),
            pending: None,
        }
    }

    pub fn maintain(&mut self) {
        let Some(pending) = self.pending.as_mut() else {
            return;
        };
        if pending.fence.settled() && pending.fence.quiesce().is_ok() {
            self.pending = None;
        } else if pending.started.elapsed() >= Duration::from_secs(5) && !pending.warned {
            pending.warned = true;
            gst::warning!(
                gst::CAT_RUST,
                "{} rollback/cleanup remains fenced; renderer requires recovery",
                self.target
            );
        }
    }

    fn replace(
        &mut self,
        pipeline: &gst::Pipeline,
        layout: Layout<'_>,
    ) -> Result<Nanoseconds, String> {
        self.maintain();
        if self.pending.is_some() {
            return Err(format!(
                "{} is fenced until the previous cancelled swap finishes rollback",
                self.target
            ));
        }
        validate_placements(layout.placements)?;
        let compositor = pipeline
            .by_name(self.target)
            .ok_or_else(|| format!("pipeline has no {}", self.target))?;
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut next = Vec::with_capacity(layout.placements.len());
        for placement in layout.placements {
            next.push(build_layer(
                pipeline,
                &compositor,
                layout.tracks,
                placement,
                deadline,
            )?);
        }
        let changes = self
            .current
            .iter()
            .map(|layer| PadChange::hidden(layer.pad.clone()))
            .chain(
                next.iter()
                    .zip(layout.placements)
                    .map(|(layer, placement)| PadChange::placement(layer.pad.clone(), placement)),
            )
            .collect();
        let mut fence = FrameSwap::arm(&compositor, changes)?;
        match fence.wait(Duration::from_secs(2)) {
            Ok(SwapResult::Confirmed(pts)) => {
                let retired = std::mem::replace(&mut self.current, next);
                if fence.quiesce().is_err() {
                    self.pending = Some(PendingCleanup {
                        fence,
                        _retired: retired,
                        started: Instant::now(),
                        warned: false,
                    });
                }
                Ok(pts)
            }
            Ok(SwapResult::Cancelled) => {
                if fence.quiesce().is_err() {
                    self.pending = Some(PendingCleanup {
                        fence,
                        _retired: next,
                        started: Instant::now(),
                        warned: false,
                    });
                }
                Err(format!(
                    "{} frame swap cancelled before visible commit; previous composition retained",
                    self.target
                ))
            }
            Err(reason) => {
                self.pending = Some(PendingCleanup {
                    fence,
                    _retired: next,
                    started: Instant::now(),
                    warned: false,
                });
                Err(reason)
            }
        }
    }
}

impl OwnerState {
    pub(super) fn prepare_composition(
        &mut self,
        revision: &ProgramRevision,
        placements: &[CompositionPlacement],
    ) -> EventKind {
        match self.preview_layers.replace(
            &self.pipeline,
            Layout {
                placements,
                tracks: &self.video_tees,
            },
        ) {
            Ok(media_at) => {
                self.prepared = None;
                self.prepared_composition = Some((revision.clone(), placements.to_vec()));
                self.applied_media(false, media_at)
            }
            Err(reason) => EventKind::Failed { reason },
        }
    }

    pub(super) fn take_prepared_composition(
        &mut self,
        revision: &ProgramRevision,
        expected_program_revision: Option<&ProgramRevision>,
    ) -> EventKind {
        let Some((prepared_revision, placements)) = self.prepared_composition.as_ref() else {
            return EventKind::Rejected {
                reason: "no prepared Preview revision".into(),
            };
        };
        if prepared_revision != revision {
            return EventKind::Rejected {
                reason: "prepared Preview revision mismatch".into(),
            };
        }
        if expected_program_revision != self.program_revision.as_ref() {
            return EventKind::Rejected {
                reason: "program revision mismatch".into(),
            };
        }
        match self.program_layers.replace(
            &self.pipeline,
            Layout {
                placements,
                tracks: &self.video_tees,
            },
        ) {
            Ok(media_at) => {
                self.program_revision = Some(revision.clone());
                self.applied_media(false, media_at)
            }
            Err(reason) => EventKind::Failed { reason },
        }
    }
}

#[cfg(test)]
#[path = "engine_frame_swap_tests.rs"]
mod frame_swap_tests;
