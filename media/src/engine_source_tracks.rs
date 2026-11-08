use std::sync::Arc;

use crate::{engine_owner::OwnerState, engine_source_runtime::SourceRuntime, tap::TapChannels};

impl OwnerState {
    pub(super) fn register_source_tracks(&mut self, runtime: &SourceRuntime) {
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

    pub(super) fn unregister_source_tracks(&mut self, runtime: &SourceRuntime) {
        for track_id in [&runtime.spec.video_track_id, &runtime.spec.audio_track_id] {
            self.tracks.remove(track_id);
            self.track_history.remove(track_id);
            self.timing_controls.remove(track_id);
            self.video_tees.remove(track_id);
            self.audio_routes.remove(track_id);
        }
    }
}
