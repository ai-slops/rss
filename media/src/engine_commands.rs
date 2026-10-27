use gst::prelude::*;
use gstreamer as gst;

use crate::engine_layout::apply_at_frame_boundary;
use crate::engine_owner::OwnerState;
use crate::protocol::{
    AudioMixRoute, CommandKind, EventKind, Milliseconds, OutputId, OutputObservedState,
    OutputSnapshot, ProgramRevision, ScenePlacement, TrackCorrection,
};
use crate::timing::{TimingLimits, TimingPlan};

const MAXIMUM_BUFFER_MS: i64 = 5_000;

impl OwnerState {
    pub(super) fn apply(&mut self, command: &CommandKind) -> EventKind {
        match command {
            CommandKind::AddSource { source } => self.add_source(source),
            CommandKind::ConfigureSource { source } => self.configure_source(source),
            CommandKind::ApplyAudioMix { routes } => self.apply_audio_mix(routes),
            CommandKind::ApplyTiming {
                program_holdback,
                video_correction,
                audio_correction,
            } => self.apply_timing((*program_holdback, *video_correction, *audio_correction)),
            CommandKind::ApplyTrackTiming {
                program_holdback,
                corrections,
            } => self.apply_track_timing(*program_holdback, corrections),
            CommandKind::PreparePreview {
                revision,
                placements,
            } => self.prepare_preview(revision, placements),
            CommandKind::PrepareComposition {
                revision,
                placements,
            } => self.prepare_composition(revision, placements),
            CommandKind::QuerySnapshot => EventKind::Snapshot {
                outputs: self.snapshot(),
            },
            CommandKind::QueryRuntime => EventKind::RuntimeSnapshot {
                sources: self.runtime_sources(),
                program: crate::engine_telemetry::program_progress(&self.pipeline),
            },
            CommandKind::ReconnectSource { source_id } => self.reconnect_source(source_id),
            CommandKind::RemoveSource { source_id } => self.remove_source(source_id),
            CommandKind::Shutdown => self.applied(false),
            CommandKind::StartOutput { output_id } => self.set_output(output_id, true),
            CommandKind::StopOutput { output_id } => self.set_output(output_id, false),
            CommandKind::TakePrepared {
                revision,
                expected_program_revision,
            } => self.take_prepared(revision, expected_program_revision.as_ref()),
        }
    }

    fn apply_audio_mix(&self, routes: &[AudioMixRoute]) -> EventKind {
        let Some(mixer) = self.pipeline.by_name("program_mixer") else {
            return EventKind::Failed {
                reason: "pipeline has no program_mixer element".to_owned(),
            };
        };
        let mut changes = Vec::with_capacity(routes.len());
        for route in routes {
            if !(-60_000..=12_000).contains(&route.gain_milli_db) {
                return EventKind::Rejected {
                    reason: format!("gain out of range for track '{}'", route.track_id.as_str()),
                };
            }
            let Some(pad_name) = self.audio_routes.get(&route.track_id) else {
                return EventKind::Rejected {
                    reason: format!("unknown audio route '{}'", route.track_id.as_str()),
                };
            };
            let Some(pad) = mixer.static_pad(pad_name) else {
                return EventKind::Failed {
                    reason: format!("program_mixer has no pad '{pad_name}'"),
                };
            };
            let volume = 10.0_f64.powf(f64::from(route.gain_milli_db) / 20_000.0);
            changes.push((pad, volume, route.muted || !route.routed));
        }
        for (pad, volume, muted) in changes {
            pad.set_property("volume", volume);
            pad.set_property("mute", muted);
        }
        self.applied(false)
    }

    pub(super) fn applied(&self, discontinuity: bool) -> EventKind {
        EventKind::Applied {
            discontinuity,
            media_at: None,
            program_revision: self.program_revision.clone(),
        }
    }

    pub(super) fn applied_media(
        &self,
        discontinuity: bool,
        media_at: crate::protocol::Nanoseconds,
    ) -> EventKind {
        EventKind::Applied {
            discontinuity,
            media_at: Some(media_at),
            program_revision: self.program_revision.clone(),
        }
    }

    fn apply_timing(&self, timing: (Milliseconds, Milliseconds, Milliseconds)) -> EventKind {
        let (holdback, video_correction, audio_correction) = timing;
        let plan = TimingPlan::validate(
            &TimingLimits::new(Milliseconds::new(MAXIMUM_BUFFER_MS)),
            holdback,
            video_correction,
            audio_correction,
        );
        let Ok(plan) = plan else {
            return rejected_timing(plan);
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

    fn apply_track_timing(
        &self,
        holdback: Milliseconds,
        corrections: &[TrackCorrection],
    ) -> EventKind {
        let mut changes = Vec::with_capacity(corrections.len());
        for correction in corrections {
            let Some(element_name) = self.tracks.get(&correction.track_id) else {
                return EventKind::Rejected {
                    reason: format!("unknown track '{}'", correction.track_id.as_str()),
                };
            };
            let plan = TimingPlan::validate(
                &TimingLimits::new(Milliseconds::new(MAXIMUM_BUFFER_MS)),
                holdback,
                correction.correction,
                correction.correction,
            );
            let Ok(plan) = plan else {
                return rejected_timing(plan);
            };
            let element = self.pipeline.by_name(element_name);
            let control = self.timing_controls.get(&correction.track_id).cloned();
            if element.is_none() && control.is_none() {
                return EventKind::Failed {
                    reason: format!("pipeline has no timing element '{element_name}'"),
                };
            }
            let delay = plan.video_delay();
            changes.push((element, control, to_nanoseconds(delay)));
        }
        for (element, control, offset) in changes {
            if let Some(control) = control {
                control.set_offset(offset);
            } else if let Some(element) = element {
                if element.find_property("ts-offset").is_some() {
                    element.set_property("ts-offset", offset);
                } else {
                    element.set_property("timestamp-offset", offset);
                }
            }
        }
        self.applied(true)
    }

    fn prepare_preview(
        &mut self,
        revision: &ProgramRevision,
        placements: &[ScenePlacement],
    ) -> EventKind {
        match apply_at_frame_boundary(&self.pipeline, "preview_compositor", placements) {
            Ok(media_at) => {
                self.prepared = Some((revision.clone(), placements.to_vec()));
                self.applied_media(false, media_at)
            }
            Err(reason) => EventKind::Failed { reason },
        }
    }

    fn take_prepared(
        &mut self,
        revision: &ProgramRevision,
        expected_program_revision: Option<&ProgramRevision>,
    ) -> EventKind {
        if expected_program_revision != self.program_revision.as_ref() {
            return EventKind::Rejected {
                reason: "program revision mismatch".to_owned(),
            };
        }
        let Some((prepared_revision, placements)) = self.prepared.as_ref() else {
            return self.take_prepared_composition(revision, expected_program_revision);
        };
        if prepared_revision != revision {
            return EventKind::Rejected {
                reason: "prepared Preview revision mismatch".to_owned(),
            };
        }
        match apply_at_frame_boundary(&self.pipeline, "program_compositor", placements) {
            Ok(media_at) => {
                self.program_revision = Some(revision.clone());
                self.applied_media(false, media_at)
            }
            Err(reason) => EventKind::Failed { reason },
        }
    }

    fn set_output(&mut self, output_id: &OutputId, running: bool) -> EventKind {
        let Some(control) = self.outputs.get_mut(output_id) else {
            return EventKind::Rejected {
                reason: format!("unknown output '{}'", output_id.as_str()),
            };
        };
        let Some(valve) = self.pipeline.by_name(&control.valve_name) else {
            return EventKind::Failed {
                reason: format!("pipeline has no valve '{}'", control.valve_name),
            };
        };
        valve.set_property("drop", !running);
        control.desired_running = running;
        control.observed = if running {
            OutputObservedState::Running
        } else {
            OutputObservedState::Stopped
        };
        self.applied(false)
    }

    fn snapshot(&self) -> Vec<OutputSnapshot> {
        let mut outputs: Vec<_> = self
            .outputs
            .iter()
            .map(|(id, control)| OutputSnapshot {
                id: id.clone(),
                desired_running: control.desired_running,
                observed: control.observed,
            })
            .collect();
        outputs.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        outputs
    }
}

fn rejected_timing(plan: Result<TimingPlan, crate::timing::TimingRejection>) -> EventKind {
    EventKind::Rejected {
        reason: plan
            .err()
            .map_or_else(|| "invalid timing".to_owned(), |error| error.to_string()),
    }
}

const fn to_nanoseconds(value: Milliseconds) -> i64 {
    value.value().saturating_mul(1_000_000)
}
