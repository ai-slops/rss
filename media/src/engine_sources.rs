use std::sync::Arc;

use gstreamer::prelude::*;

use crate::engine_owner::{OwnerState, now_nanoseconds};
use crate::engine_source_graph::validate_source;
use crate::engine_source_runtime::SourceRuntime;
use crate::protocol::{
    ContinuityGeneration, EventKind, GapTimebase, Nanoseconds, SourceGap, SourceId, SourceSnapshot,
    SourceSpec,
};
use crate::tap::TapChannels;

impl OwnerState {
    pub(super) fn add_source(&mut self, spec: &SourceSpec) -> EventKind {
        if self.sources.contains_key(&spec.id) {
            return EventKind::Rejected {
                reason: format!("source '{}' already exists", spec.id.as_str()),
            };
        }
        match SourceRuntime::build(
            &self.pipeline,
            &self.engine_run_id,
            spec.clone(),
            1,
            Vec::new(),
        ) {
            Ok(runtime) => {
                self.register_source_tracks(&runtime);
                self.sources.insert(spec.id.clone(), runtime);
                self.applied(false)
            }
            Err(reason) => EventKind::Rejected { reason },
        }
    }

    pub(super) fn remove_source(&mut self, source_id: &SourceId) -> EventKind {
        let Some(runtime) = self.sources.remove(source_id) else {
            return EventKind::Rejected {
                reason: format!("unknown source '{}'", source_id.as_str()),
            };
        };
        self.unregister_source_tracks(&runtime);
        match runtime.detach(&self.pipeline) {
            Ok(()) => self.applied(false),
            Err(reason) => EventKind::Failed { reason },
        }
    }

    pub(super) fn reconnect_source(&mut self, source_id: &SourceId) -> EventKind {
        let Some(previous) = self.sources.remove(source_id) else {
            return EventKind::Rejected {
                reason: format!("unknown source '{}'", source_id.as_str()),
            };
        };
        let spec = previous.spec.clone();
        self.rebuild_source(previous, &spec)
    }

    pub(super) fn configure_source(&mut self, spec: &SourceSpec) -> EventKind {
        let Some(previous) = self.sources.remove(&spec.id) else {
            return EventKind::Rejected {
                reason: format!("unknown source '{}'", spec.id.as_str()),
            };
        };
        if previous.spec.video_track_id != spec.video_track_id
            || previous.spec.audio_track_id != spec.audio_track_id
        {
            self.sources.insert(spec.id.clone(), previous);
            return EventKind::Rejected {
                reason: "source track identity cannot change during configuration".to_owned(),
            };
        }
        if let Err(reason) = validate_source(spec) {
            self.sources.insert(spec.id.clone(), previous);
            return EventKind::Rejected { reason };
        }
        self.rebuild_source(previous, spec)
    }

    fn rebuild_source(&mut self, previous: SourceRuntime, spec: &SourceSpec) -> EventKind {
        let previous_spec = previous.spec.clone();
        let video_offset = previous.video_timing_control.offset();
        let audio_offset = previous.audio_timing_control.offset();
        self.unregister_source_tracks(&previous);
        let generation = previous.generation.saturating_add(1);
        let mut gaps = previous.gaps.clone();
        gaps.push(SourceGap {
            timebase: GapTimebase::EngineMedia,
            generation: ContinuityGeneration::new(generation),
            started_at: media_time(&self.pipeline),
            ended_at: None,
        });
        if let Err(reason) = previous.detach(&self.pipeline) {
            return EventKind::Failed { reason };
        }
        match SourceRuntime::build(
            &self.pipeline,
            &self.engine_run_id,
            spec.clone(),
            generation,
            gaps.clone(),
        ) {
            Ok(runtime) => {
                runtime.video_timing_control.set_offset(video_offset);
                runtime.audio_timing_control.set_offset(audio_offset);
                self.register_source_tracks(&runtime);
                self.sources.insert(spec.id.clone(), runtime);
                self.applied(true)
            }
            Err(reason) => {
                if let Ok(runtime) = SourceRuntime::build(
                    &self.pipeline,
                    &self.engine_run_id,
                    previous_spec,
                    generation,
                    gaps,
                ) {
                    runtime.video_timing_control.set_offset(video_offset);
                    runtime.audio_timing_control.set_offset(audio_offset);
                    self.register_source_tracks(&runtime);
                    self.sources.insert(spec.id.clone(), runtime);
                }
                EventKind::Failed { reason }
            }
        }
    }

    pub(super) fn runtime_sources(&mut self) -> Vec<SourceSnapshot> {
        let now = now_nanoseconds();
        let media_now = media_time(&self.pipeline);
        let mut snapshots: Vec<_> = self
            .sources
            .values_mut()
            .map(|runtime| runtime.snapshot(&self.pipeline, now, media_now))
            .collect();
        snapshots.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        snapshots
    }

    fn register_source_tracks(&mut self, runtime: &SourceRuntime) {
        self.tracks.insert(
            runtime.spec.video_track_id.clone(),
            runtime.video_timing_name.clone(),
        );
        self.tracks.insert(
            runtime.spec.audio_track_id.clone(),
            runtime.audio_timing_name.clone(),
        );
        self.track_history.insert(
            runtime.spec.video_track_id.clone(),
            runtime.video_history_name.clone(),
        );
        self.timing_controls.insert(
            runtime.spec.video_track_id.clone(),
            Arc::clone(&runtime.video_timing_control),
        );
        self.timing_controls.insert(
            runtime.spec.audio_track_id.clone(),
            Arc::clone(&runtime.audio_timing_control),
        );
        self.track_history.insert(
            runtime.spec.audio_track_id.clone(),
            runtime.audio_history_name.clone(),
        );
        self.video_tees.insert(
            runtime.spec.video_track_id.clone(),
            TapChannels::source(
                &self.engine_run_id,
                &runtime.spec.video_track_id,
                &runtime.spec.audio_track_id,
            )
            .video,
        );
        if let Some(audio_mix) = &runtime.audio_mix {
            if let Some(route) = self.mix_settings.get(&runtime.spec.audio_track_id) {
                audio_mix.restore_mix(route);
            }
            self.audio_routes.insert(
                runtime.spec.audio_track_id.clone(),
                audio_mix.mixer_pad_name(),
            );
        }
    }

    fn unregister_source_tracks(&mut self, runtime: &SourceRuntime) {
        for track_id in [&runtime.spec.video_track_id, &runtime.spec.audio_track_id] {
            self.tracks.remove(track_id);
            self.track_history.remove(track_id);
            self.timing_controls.remove(track_id);
            self.video_tees.remove(track_id);
            self.audio_routes.remove(track_id);
        }
    }
}

fn media_time(pipeline: &gstreamer::Pipeline) -> Nanoseconds {
    Nanoseconds::new(
        pipeline
            .current_running_time()
            .map_or(0, gstreamer::ClockTime::nseconds),
    )
}
