use std::sync::Arc;

use gst::prelude::*;
use gstreamer as gst;

use crate::engine_source_audio::AudioMixRuntime;
use crate::engine_source_graph::{SourceNames, element_stem, source_description, validate_source};
use crate::engine_source_progress::{
    TimingControl, TrackProgress, add_progress_probe, add_timestamp_normalizer, add_timing_probe,
    queue_level,
};
use crate::protocol::{
    ContinuityGeneration, EngineRunId, FrameCount, GapTimebase, Nanoseconds, SourceGap,
    SourceSnapshot, SourceSpec,
};

pub struct SourceRuntime {
    pub spec: SourceSpec,
    bin: gst::Bin,
    pub generation: u64,
    pub gaps: Vec<SourceGap>,
    video_progress: Arc<TrackProgress>,
    audio_progress: Arc<TrackProgress>,
    video_ingest_progress: Arc<TrackProgress>,
    audio_ingest_progress: Arc<TrackProgress>,
    pub video_timing_control: Arc<TimingControl>,
    pub audio_timing_control: Arc<TimingControl>,
    pub video_history_name: String,
    pub audio_history_name: String,
    pub video_timing_name: String,
    pub audio_timing_name: String,
    pub audio_mix: Option<AudioMixRuntime>,
    was_connected: bool,
}

impl SourceRuntime {
    pub fn build(
        pipeline: &gst::Pipeline,
        engine_run_id: &EngineRunId,
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
        let video_ingest = bin
            .by_name(&names.video_ingest)
            .and_then(|element| element.static_pad("src"))
            .ok_or_else(|| "source video has no reference timestamp pad".to_owned())?;
        crate::tap_timing::tag(&video_ingest, engine_run_id)?;
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

    pub fn detach(self, pipeline: &gst::Pipeline) -> Result<(), String> {
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

    pub fn snapshot(
        &mut self,
        pipeline: &gst::Pipeline,
        wall_now: Nanoseconds,
        media_now: Nanoseconds,
    ) -> SourceSnapshot {
        let last_activity = self
            .video_progress
            .last_activity_ns()
            .max(self.audio_progress.last_activity_ns());
        let connected =
            last_activity != 0 && wall_now.value().saturating_sub(last_activity) <= 2_000_000_000;
        if connected && !self.was_connected {
            if let Some(gap) = self.gaps.last_mut().filter(|gap| gap.ended_at.is_none()) {
                gap.ended_at = Some(media_now);
            }
        } else if !connected && self.was_connected {
            self.gaps.push(SourceGap {
                timebase: GapTimebase::EngineMedia,
                generation: ContinuityGeneration::new(self.generation),
                started_at: media_now,
                ended_at: None,
            });
        }
        self.was_connected = connected;
        let audio_activity = self.audio_progress.last_activity_ns();
        let meters = self
            .audio_mix
            .as_ref()
            .filter(|_| {
                audio_activity != 0
                    && wall_now.value().saturating_sub(audio_activity) <= 2_000_000_000
            })
            .map(AudioMixRuntime::meters);
        SourceSnapshot {
            id: self.spec.id.clone(),
            continuity_generation: ContinuityGeneration::new(self.generation),
            connected,
            video_received_frames: FrameCount::new(self.video_ingest_progress.buffers()),
            audio_received_buffers: FrameCount::new(self.audio_ingest_progress.buffers()),
            video_received_pts: self
                .video_ingest_progress
                .last_pts_ns()
                .map(Nanoseconds::new),
            video_playout_pts: self.video_progress.last_pts_ns().map(Nanoseconds::new),
            video_delay: crate::protocol::Milliseconds::new(
                self.video_timing_control.offset() / 1_000_000,
            ),
            audio_delay: crate::protocol::Milliseconds::new(
                self.audio_timing_control.offset() / 1_000_000,
            ),
            video_frames: FrameCount::new(self.video_progress.buffers()),
            audio_buffers: FrameCount::new(self.audio_progress.buffers()),
            audio_input_peak_milli_db: meters.as_ref().and_then(|meter| meter.input),
            audio_post_peak_milli_db: meters.as_ref().and_then(|meter| meter.post),
            audio_clipping: meters.as_ref().and_then(|meter| meter.clipping),
            video_buffered_ns: Nanoseconds::new(queue_level(pipeline, &self.video_history_name)),
            audio_buffered_ns: Nanoseconds::new(queue_level(pipeline, &self.audio_history_name)),
            gaps: self.gaps.clone(),
        }
    }
}
