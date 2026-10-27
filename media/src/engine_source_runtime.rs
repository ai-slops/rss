use std::sync::Arc;

use gst::prelude::*;
use gstreamer as gst;

use crate::engine_source_admission::SourceAdmission;
use crate::engine_source_audio::AudioMixRuntime;
use crate::engine_source_drift::{DriftObserver, install_drift_probes};
use crate::engine_source_epoch::{SourceEpochPair, SourceEpochSetup, install_source_epochs};
use crate::engine_source_graph::{SourceNames, element_stem, source_description, validate_source};
use crate::engine_source_progress::{
    ProgressNames, TimingControl, TrackProgress, add_timing_probe, install_progress_probes,
    queue_level,
};
use crate::protocol::{
    ContinuityGeneration, EngineRunId, FrameCount, GapTimebase, Nanoseconds, SourceGap, SourceKind,
    SourceSnapshot, SourceSpec,
};

pub struct SourceRuntime {
    pub spec: SourceSpec,
    pub(crate) bin: gst::Bin,
    pub generation: u64,
    pub gaps: Vec<SourceGap>,
    video_progress: Arc<TrackProgress>,
    audio_progress: Arc<TrackProgress>,
    video_ingest_progress: Arc<TrackProgress>,
    audio_ingest_progress: Arc<TrackProgress>,
    video_drift: Arc<DriftObserver>,
    audio_drift: Arc<DriftObserver>,
    pub video_timing_control: Arc<TimingControl>,
    pub audio_timing_control: Arc<TimingControl>,
    pub video_history_name: String,
    pub audio_history_name: String,
    pub video_timing_name: String,
    pub audio_timing_name: String,
    pub audio_mix: Option<AudioMixRuntime>,
    pub(crate) admission: Option<Arc<SourceAdmission>>,
    pub(crate) epochs: SourceEpochPair,
    pub(crate) srt_listener_name: Option<String>,
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
        let admission = match &spec.kind {
            SourceKind::SrtListener { passphrase, .. } => Some(SourceAdmission::install(
                &bin,
                &names.srt_listener,
                generation,
                engine_run_id,
                &spec.id,
                passphrase,
            )?),
            SourceKind::Synthetic { .. } => None,
        };
        let video_timing_control = Arc::new(TimingControl::default());
        let audio_timing_control = Arc::new(TimingControl::default());
        let (video_drift, audio_drift) =
            install_drift_probes(&bin, &names.video_ingest, &names.audio_ingest)?;
        let progress = install_progress_probes(
            &bin,
            ProgressNames {
                video_playout: &names.video_playout,
                audio_playout: &names.audio_playout,
                video_ingest: &names.video_ingest,
                audio_ingest: &names.audio_ingest,
            },
        )?;
        let epochs = install_source_epochs(&SourceEpochSetup {
            bin: &bin,
            names: &names,
            pipeline,
            run: engine_run_id,
            spec: &spec,
            generation,
        })?;
        add_timing_probe(&bin, &names.video_timing, Arc::clone(&video_timing_control))?;
        add_timing_probe(&bin, &names.audio_timing, Arc::clone(&audio_timing_control))?;
        pipeline
            .add(&bin)
            .map_err(|error| format!("attach source '{}': {error}", spec.id.as_str()))?;
        bin.sync_state_with_parent()
            .map_err(|error| format!("start source '{}': {error}", spec.id.as_str()))?;
        let audio_mix = AudioMixRuntime::build(pipeline, &spec, engine_run_id)?;
        let srt_listener_name = matches!(&spec.kind, SourceKind::SrtListener { .. })
            .then(|| names.srt_listener.clone());
        Ok(Self {
            spec,
            bin,
            generation,
            gaps,
            video_progress: progress.video_playout,
            audio_progress: progress.audio_playout,
            video_ingest_progress: progress.video_ingest,
            audio_ingest_progress: progress.audio_ingest,
            video_drift,
            audio_drift,
            video_timing_control,
            audio_timing_control,
            video_history_name: names.video_history,
            audio_history_name: names.audio_history,
            video_timing_name: names.video_timing,
            audio_timing_name: names.audio_timing,
            audio_mix,
            admission,
            epochs,
            srt_listener_name,
            was_connected: false,
        })
    }

    pub fn detach(self, pipeline: &gst::Pipeline) -> Result<(), String> {
        self.bin
            .set_state(gst::State::Null)
            .map_err(|error| format!("stop source '{}': {error:?}", self.spec.id.as_str()))?;
        let (_, state, _) = self.bin.state(gst::ClockTime::from_seconds(5));
        if state != gst::State::Null {
            return Err(format!(
                "source '{}' did not reach NULL",
                self.spec.id.as_str()
            ));
        }
        if let Some(admission) = &self.admission {
            admission.disconnect();
        }
        self.epochs.disconnect();
        if let Some(audio_mix) = self.audio_mix {
            audio_mix.detach(pipeline)?;
        }
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
        self.generation = self.current_generation();
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
            video_drift: self.video_drift.estimate().map(Into::into),
            audio_drift: self.audio_drift.estimate().map(Into::into),
            video_buffered_ns: Nanoseconds::new(queue_level(pipeline, &self.video_history_name)),
            audio_buffered_ns: Nanoseconds::new(queue_level(pipeline, &self.audio_history_name)),
            gaps: self.gaps.clone(),
        }
    }
}
