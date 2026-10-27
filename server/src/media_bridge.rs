use live_production_media::{
    engine::MediaEngine,
    protocol::{
        AudioMixRoute, Command, CommandId, CommandKind, CompositionPlacement, EngineRunId,
        EventKind, Milliseconds, Nanoseconds, ProgramRevision, TrackCorrection, TrackId,
    },
};

use crate::error::{AppError, Result};
use num_traits::ToPrimitive;

pub use crate::media_bridge_requests::{
    AudioRequest, PrepareRequest, TakeRequest, TrackTimingRequest,
};

pub struct MediaBridge {
    engine: MediaEngine,
    engine_run_id: EngineRunId,
}

pub struct AppliedMedia {
    pub at_ns: Nanoseconds,
    pub discontinuity: bool,
    pub program_revision: Option<String>,
}

impl MediaBridge {
    pub fn spawn() -> Result<Self> {
        let engine = MediaEngine::spawn(crate::media_bridge_pipeline::engine_config()?)
            .map_err(|error| AppError::Unavailable(error.to_string()))?;
        let event = engine
            .execute(Command {
                id: command_id(&uuid::Uuid::new_v4().to_string())?,
                expected_engine_run_id: None,
                kind: CommandKind::QuerySnapshot,
            })
            .map_err(|error| AppError::Unavailable(error.to_string()))?;
        Ok(Self {
            engine,
            engine_run_id: event.engine_run_id,
        })
    }

    #[must_use]
    pub fn engine_run_id(&self) -> &str {
        self.engine_run_id.as_str()
    }

    #[must_use]
    pub(crate) fn typed_engine_run_id(&self) -> &EngineRunId {
        &self.engine_run_id
    }

    pub(crate) fn execute_kind(
        &self,
        command_id_value: &str,
        expected_engine_run_id: &str,
        kind: CommandKind,
    ) -> Result<live_production_media::protocol::EngineEvent> {
        self.engine
            .execute(Command {
                id: command_id(command_id_value)?,
                expected_engine_run_id: Some(engine_run_id(expected_engine_run_id)?),
                kind,
            })
            .map_err(|error| AppError::Unavailable(error.to_string()))
    }

    pub fn initialize_program(
        &self,
        revision: &str,
        placements: Vec<CompositionPlacement>,
    ) -> Result<AppliedMedia> {
        self.take(TakeRequest {
            command_id: &uuid::Uuid::new_v4().to_string(),
            expected_engine_run_id: self.engine_run_id(),
            revision,
            expected_program_revision: None,
            placements,
        })
    }

    pub fn apply_track_timing(&self, request: TrackTimingRequest<'_>) -> Result<AppliedMedia> {
        let corrections = request
            .corrections
            .into_iter()
            .map(|(track_id, correction)| {
                Ok(TrackCorrection {
                    track_id: track_id_value(&track_id)?,
                    correction: Milliseconds::new(correction),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let event = self
            .engine
            .execute(Command {
                id: command_id(request.command_id)?,
                expected_engine_run_id: Some(engine_run_id(request.expected_engine_run_id)?),
                kind: CommandKind::ApplyTrackTiming {
                    program_holdback: Milliseconds::new(request.holdback_ms),
                    corrections,
                },
            })
            .map_err(|error| AppError::Unavailable(error.to_string()))?;
        applied_event(event)
    }

    pub fn apply_audio(&self, request: AudioRequest<'_>) -> Result<AppliedMedia> {
        let routes = request
            .routes
            .iter()
            .map(|route| {
                let gain_milli_db = (route.gain_db * 1_000.0)
                    .round()
                    .to_i32()
                    .ok_or_else(|| AppError::Invalid("Audio gain is out of range.".into()))?;
                Ok(AudioMixRoute {
                    track_id: track_id_value(&route.track_id.0)?,
                    gain_milli_db,
                    muted: route.muted,
                    routed: route.routed,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let event = self
            .engine
            .execute(Command {
                id: command_id(request.command_id)?,
                expected_engine_run_id: Some(engine_run_id(request.expected_engine_run_id)?),
                kind: CommandKind::ApplyAudioMix { routes },
            })
            .map_err(|error| AppError::Unavailable(error.to_string()))?;
        applied_event(event)
    }

    pub fn take(&self, request: TakeRequest<'_>) -> Result<AppliedMedia> {
        let revision = program_revision(request.revision)?;
        self.prepare(PrepareRequest {
            command_id: &uuid::Uuid::new_v4().to_string(),
            expected_engine_run_id: request.expected_engine_run_id,
            revision: request.revision,
            placements: request.placements,
        })?;
        let event = self
            .engine
            .execute(Command {
                id: command_id(request.command_id)?,
                expected_engine_run_id: Some(engine_run_id(request.expected_engine_run_id)?),
                kind: CommandKind::TakePrepared {
                    revision,
                    expected_program_revision: request
                        .expected_program_revision
                        .map(program_revision)
                        .transpose()?,
                },
            })
            .map_err(|error| AppError::Unavailable(error.to_string()))?;
        applied_event(event)
    }

    pub fn prepare(&self, request: PrepareRequest<'_>) -> Result<AppliedMedia> {
        let event = self
            .engine
            .execute(Command {
                id: command_id(request.command_id)?,
                expected_engine_run_id: Some(engine_run_id(request.expected_engine_run_id)?),
                kind: CommandKind::PrepareComposition {
                    revision: program_revision(request.revision)?,
                    placements: request.placements,
                },
            })
            .map_err(|error| AppError::Unavailable(error.to_string()))?;
        applied_event(event)
    }

    pub fn shutdown(&self) {
        let Ok(id) = command_id(&uuid::Uuid::new_v4().to_string()) else {
            return;
        };
        let _ = self.engine.execute(Command {
            id,
            expected_engine_run_id: Some(self.engine_run_id.clone()),
            kind: CommandKind::Shutdown,
        });
    }
}

impl Drop for MediaBridge {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn command_id(value: &str) -> Result<CommandId> {
    CommandId::new(value).map_err(|error| AppError::Invalid(error.to_string()))
}

fn engine_run_id(value: &str) -> Result<EngineRunId> {
    EngineRunId::new(value).map_err(|error| AppError::Invalid(error.to_string()))
}

fn program_revision(value: &str) -> Result<ProgramRevision> {
    ProgramRevision::new(value).map_err(|error| AppError::Invalid(error.to_string()))
}

fn track_id_value(value: &str) -> Result<TrackId> {
    TrackId::new(value).map_err(|error| AppError::Invalid(error.to_string()))
}

pub(crate) fn applied_event(
    event: live_production_media::protocol::EngineEvent,
) -> Result<AppliedMedia> {
    match event.kind {
        EventKind::Applied {
            discontinuity,
            media_at,
            program_revision,
        } => Ok(AppliedMedia {
            at_ns: media_at.unwrap_or(event.at),
            discontinuity,
            program_revision: program_revision.map(|revision| revision.as_str().to_owned()),
        }),
        EventKind::Rejected { reason } => Err(AppError::Conflict(reason)),
        EventKind::Failed { reason } => Err(AppError::Unavailable(reason)),
        EventKind::Accepted | EventKind::Snapshot { .. } | EventKind::RuntimeSnapshot { .. } => {
            Err(AppError::Internal(anyhow::anyhow!(
                "unexpected media response"
            )))
        }
    }
}
