use std::time::Duration;

use live_production_media::output::{
    DestinationUrl, MediaTap, OutputDefinition, OutputId, OutputManager, RecordingDefinition,
};

use crate::{
    error::{AppError, Result},
    model::{Output, OutputAdapter, Project, TrackKind},
    store::Store,
};

pub(crate) fn register(
    manager: &OutputManager,
    output: &Output,
    project: &Project,
    store: &Store,
    state_directory: &std::path::Path,
    engine_run_id: &live_production_media::protocol::EngineRunId,
) -> Result<()> {
    let id = OutputId::new(&output.id.0).map_err(invalid)?;
    let definition = match &output.adapter {
        OutputAdapter::ProgramRecording { segment_seconds } => OutputDefinition::Recording(
            RecordingDefinition::new(
                id,
                program_tap(engine_run_id)?,
                &state_directory.join("recordings").join(&output.id.0),
                Duration::from_secs(u64::from(*segment_seconds)),
            )
            .map_err(invalid)?,
        ),
        OutputAdapter::SourceRecording {
            source_id,
            segment_seconds,
        } => {
            let source = project
                .sources
                .iter()
                .find(|source| source.id == *source_id)
                .ok_or_else(|| AppError::Invalid("Recording source no longer exists.".into()))?;
            let video = source
                .tracks
                .iter()
                .find(|track| track.kind == TrackKind::Video)
                .ok_or_else(|| AppError::Invalid("Recording source has no video track.".into()))?;
            let audio = source
                .tracks
                .iter()
                .find(|track| track.kind == TrackKind::Audio)
                .ok_or_else(|| AppError::Invalid("Recording source has no audio track.".into()))?;
            let channels = live_production_media::tap::TapChannels::clean(
                engine_run_id,
                &live_production_media::protocol::TrackId::new(&video.id.0)
                    .map_err(|error| AppError::Invalid(error.to_string()))?,
                &live_production_media::protocol::TrackId::new(&audio.id.0)
                    .map_err(|error| AppError::Invalid(error.to_string()))?,
            );
            let tap = MediaTap::new(&channels.video, &channels.audio).map_err(invalid)?;
            OutputDefinition::Recording(
                RecordingDefinition::new(
                    id,
                    tap,
                    &state_directory.join("recordings").join(&output.id.0),
                    Duration::from_secs(u64::from(*segment_seconds)),
                )
                .map_err(invalid)?,
            )
        }
        OutputAdapter::Rtmps { destination_id } => {
            let url = store
                .destination_get(&destination_id.0)
                .map_err(AppError::Internal)?
                .ok_or_else(|| {
                    AppError::Invalid(
                        "Provision this destination before creating the output.".into(),
                    )
                })?;
            OutputDefinition::rtmps(
                id,
                program_tap(engine_run_id)?,
                DestinationUrl::new(&url).map_err(invalid)?,
            )
        }
    };
    manager.register(definition).map_err(unavailable)
}

fn program_tap(engine_run_id: &live_production_media::protocol::EngineRunId) -> Result<MediaTap> {
    let channels = live_production_media::tap::TapChannels::program(engine_run_id);
    MediaTap::new(&channels.video, &channels.audio).map_err(invalid)
}

fn invalid(error: live_production_media::output::OutputError) -> AppError {
    AppError::Invalid(error.to_string())
}

fn unavailable(error: live_production_media::output::OutputError) -> AppError {
    AppError::Unavailable(error.to_string())
}
