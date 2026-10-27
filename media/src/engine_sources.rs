use std::sync::Arc;

use gst::prelude::*;
use gstreamer as gst;

use crate::engine_owner::{OwnerState, now_nanoseconds};
use crate::engine_source_audio::AudioMixRuntime;
use crate::engine_source_graph::{SourceNames, element_stem, source_description, validate_source};
use crate::engine_source_progress::{
    TimingControl, TrackProgress, add_progress_probe, add_timestamp_normalizer, add_timing_probe,
    queue_level,
};
use crate::protocol::{
    ContinuityGeneration, EventKind, FrameCount, Nanoseconds, SourceGap, SourceId, SourceSnapshot,
    SourceSpec,
};
use crate::tap::TapChannels;

pub struct SourceRuntime {
    spec: SourceSpec,
    bin: gst::Bin,
    generation: u64,
    gaps: Vec<SourceGap>,
    video_progress: Arc<TrackProgress>,
    audio_progress: Arc<TrackProgress>,
    video_ingest_progress: Arc<TrackProgress>,
    audio_ingest_progress: Arc<TrackProgress>,
    video_timing_control: Arc<TimingControl>,
    audio_timing_control: Arc<TimingControl>,
    video_history_name: String,
    audio_history_name: String,
    video_timing_name: String,
    audio_timing_name: String,
    audio_mix: Option<AudioMixRuntime>,
    was_connected: bool,
}

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
        self.unregister_source_tracks(&previous);
        let generation = previous.generation.saturating_add(1);
        let mut gaps = previous.gaps.clone();
        gaps.push(SourceGap {
            generation: ContinuityGeneration::new(generation),
            started_at: now_nanoseconds(),
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
                    self.register_source_tracks(&runtime);
                    self.sources.insert(spec.id.clone(), runtime);
                }
                EventKind::Failed { reason }
            }
        }
    }

    pub(super) fn runtime_sources(&mut self) -> Vec<SourceSnapshot> {
        let now = now_nanoseconds();
        let mut snapshots: Vec<_> = self
            .sources
            .values_mut()
            .map(|runtime| runtime.snapshot(&self.pipeline, now))
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

impl SourceRuntime {
    fn build(
        pipeline: &gst::Pipeline,
        engine_run_id: &crate::protocol::EngineRunId,
        spec: SourceSpec,
        generation: u64,
        gaps: Vec<SourceGap>,
    ) -> Result<Self, String> {
        validate_source(&spec)?;
        let stem = element_stem(spec.id.as_str());
        let names = SourceNames::new(&stem);
        let description = source_description(&spec, &names, engine_run_id);
        let bin = gst::parse::bin_from_description_with_name(
            &description,
            false,
            &format!("source_bin_{stem}"),
        )
        .map_err(|error| format!("build source '{}': {error}", spec.id.as_str()))?;
        let video_progress = Arc::new(TrackProgress::default());
        let audio_progress = Arc::new(TrackProgress::default());
        let video_ingest_progress = Arc::new(TrackProgress::default());
        let audio_ingest_progress = Arc::new(TrackProgress::default());
        let video_timing_control = Arc::new(TimingControl::default());
        let audio_timing_control = Arc::new(TimingControl::default());
        add_timestamp_normalizer(&bin, &names.video_ingest, pipeline)?;
        add_timestamp_normalizer(&bin, &names.audio_ingest, pipeline)?;
        add_timing_probe(&bin, &names.video_timing, Arc::clone(&video_timing_control))?;
        add_timing_probe(&bin, &names.audio_timing, Arc::clone(&audio_timing_control))?;
        add_progress_probe(&bin, &names.video_playout, Arc::clone(&video_progress))?;
        add_progress_probe(&bin, &names.audio_playout, Arc::clone(&audio_progress))?;
        add_progress_probe(
            &bin,
            &names.video_ingest,
            Arc::clone(&video_ingest_progress),
        )?;
        add_progress_probe(
            &bin,
            &names.audio_ingest,
            Arc::clone(&audio_ingest_progress),
        )?;
        pipeline
            .add(&bin)
            .map_err(|error| format!("attach source '{}': {error}", spec.id.as_str()))?;
        bin.sync_state_with_parent()
            .map_err(|error| format!("start source '{}': {error}", spec.id.as_str()))?;
        let audio_mix = AudioMixRuntime::build(pipeline, &spec, engine_run_id)?;
        Ok(Self {
            spec,
            bin,
            generation,
            gaps,
            video_progress,
            audio_progress,
            video_ingest_progress,
            audio_ingest_progress,
            video_timing_control,
            audio_timing_control,
            video_history_name: names.video_history,
            audio_history_name: names.audio_history,
            video_timing_name: names.video_timing,
            audio_timing_name: names.audio_timing,
            audio_mix,
            was_connected: false,
        })
    }

    fn detach(self, pipeline: &gst::Pipeline) -> Result<(), String> {
        if let Some(audio_mix) = self.audio_mix {
            audio_mix.detach(pipeline)?;
        }
        self.bin
            .set_state(gst::State::Null)
            .map_err(|error| format!("stop source '{}': {error:?}", self.spec.id.as_str()))?;
        pipeline
            .remove(&self.bin)
            .map_err(|error| format!("detach source '{}': {error}", self.spec.id.as_str()))
    }

    fn snapshot(&mut self, pipeline: &gst::Pipeline, now: Nanoseconds) -> SourceSnapshot {
        let video_frames = self.video_progress.buffers();
        let audio_buffers = self.audio_progress.buffers();
        let last_activity = self
            .video_progress
            .last_activity_ns()
            .max(self.audio_progress.last_activity_ns());
        let connected =
            last_activity != 0 && now.value().saturating_sub(last_activity) <= 2_000_000_000;
        if connected && !self.was_connected {
            if let Some(gap) = self.gaps.last_mut().filter(|gap| gap.ended_at.is_none()) {
                gap.ended_at = Some(now);
            }
        } else if !connected && self.was_connected {
            self.gaps.push(SourceGap {
                generation: ContinuityGeneration::new(self.generation),
                started_at: now,
                ended_at: None,
            });
        }
        self.was_connected = connected;
        SourceSnapshot {
            id: self.spec.id.clone(),
            continuity_generation: ContinuityGeneration::new(self.generation),
            connected,
            video_received_frames: FrameCount::new(self.video_ingest_progress.buffers()),
            audio_received_buffers: FrameCount::new(self.audio_ingest_progress.buffers()),
            video_frames: FrameCount::new(video_frames),
            audio_buffers: FrameCount::new(audio_buffers),
            video_buffered_ns: Nanoseconds::new(queue_level(pipeline, &self.video_history_name)),
            audio_buffered_ns: Nanoseconds::new(queue_level(pipeline, &self.audio_history_name)),
            gaps: self.gaps.clone(),
        }
    }
}
