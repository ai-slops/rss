use gst::prelude::*;
use gstreamer as gst;

use crate::engine_layout::apply_at_frame_boundary;
use crate::engine_owner::OwnerState;
use crate::protocol::{
    AudioMixRoute, CommandKind, EventKind, OutputId, OutputObservedState, OutputSnapshot,
    ProgramRevision, ScenePlacement,
};

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
                clock: crate::engine_telemetry::clock_mapping(&self.pipeline),
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

    fn apply_audio_mix(&mut self, routes: &[AudioMixRoute]) -> EventKind {
        let Some(mixer) = self.pipeline.by_name("program_mixer") else {
            return EventKind::Failed {
                reason: "pipeline has no program_mixer element".to_owned(),
            };
        };
        let mut identities = std::collections::HashSet::new();
        for route in routes {
            if !identities.insert(&route.track_id) {
                return EventKind::Rejected {
                    reason: "audio tracks may be routed only once".into(),
                };
            }
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
            if mixer.static_pad(pad_name).is_none() {
                return EventKind::Failed {
                    reason: format!("program_mixer has no pad '{pad_name}'"),
                };
            }
        }
        let mut changes = Vec::with_capacity(self.audio_routes.len());
        for (track, name) in &self.audio_routes {
            let Some(pad) = mixer.static_pad(name) else {
                return EventKind::Failed {
                    reason: format!("program mixer lost route '{}'", track.as_str()),
                };
            };
            let route = routes.iter().find(|route| &route.track_id == track);
            let volume = route.map_or(1.0, |route| {
                10.0_f64.powf(f64::from(route.gain_milli_db) / 20_000.0)
            });
            let muted = route.is_none_or(|route| route.muted || !route.routed);
            changes.push((pad, volume, muted));
        }
        for (pad, volume, muted) in changes {
            pad.set_property("volume", volume);
            pad.set_property("mute", muted);
        }
        self.mix_settings = routes
            .iter()
            .map(|route| (route.track_id.clone(), route.clone()))
            .collect();
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
