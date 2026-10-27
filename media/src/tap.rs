use crate::protocol::{EngineRunId, TrackId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TapChannels {
    pub video: String,
    pub audio: String,
}

impl TapChannels {
    #[must_use]
    pub fn program(engine_run_id: &EngineRunId) -> Self {
        Self::pair(engine_run_id, "program", None)
    }

    #[must_use]
    pub fn preview(engine_run_id: &EngineRunId) -> String {
        format!("live-{}-preview-video-v1", engine_run_id.as_str())
    }

    #[must_use]
    pub fn source(
        engine_run_id: &EngineRunId,
        video_track_id: &TrackId,
        audio_track_id: &TrackId,
    ) -> Self {
        Self {
            video: Self::source_video(engine_run_id, video_track_id),
            audio: Self::source_audio(engine_run_id, audio_track_id),
        }
    }

    #[must_use]
    pub fn source_video(engine_run_id: &EngineRunId, track_id: &TrackId) -> String {
        format!(
            "live-{}-source-video-v1-{}",
            engine_run_id.as_str(),
            track_id.as_str()
        )
    }

    #[must_use]
    pub fn source_audio(engine_run_id: &EngineRunId, track_id: &TrackId) -> String {
        format!(
            "live-{}-source-audio-v1-{}",
            engine_run_id.as_str(),
            track_id.as_str()
        )
    }

    #[must_use]
    pub fn clean(
        engine_run_id: &EngineRunId,
        video_track_id: &TrackId,
        audio_track_id: &TrackId,
    ) -> Self {
        Self {
            video: format!(
                "live-{}-clean-video-v1-{}",
                engine_run_id.as_str(),
                video_track_id.as_str()
            ),
            audio: format!(
                "live-{}-clean-audio-v1-{}",
                engine_run_id.as_str(),
                audio_track_id.as_str()
            ),
        }
    }

    fn pair(engine_run_id: &EngineRunId, kind: &str, suffix: Option<&str>) -> Self {
        let suffix = suffix.map_or(String::new(), |value| format!("-{value}"));
        Self {
            video: format!("live-{}-{kind}-video-v1{suffix}", engine_run_id.as_str()),
            audio: format!("live-{}-{kind}-audio-v1{suffix}", engine_run_id.as_str()),
        }
    }
}
