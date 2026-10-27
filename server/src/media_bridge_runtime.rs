use live_production_media::protocol::{
    CommandKind, CompositionPlacement, EngineClockMapping, EventKind, ProgramProgress,
    ProgramRevision, SourceId, SourceSnapshot, SourceSpec,
};

use crate::{
    error::{AppError, Result},
    media_bridge::{AppliedMedia, MediaBridge, applied_event},
};

pub(crate) struct CompositionRequest<'a> {
    pub(crate) command_id: &'a str,
    pub(crate) expected_engine_run_id: &'a str,
    pub(crate) revision: &'a str,
    pub(crate) expected_program_revision: Option<&'a str>,
    pub(crate) placements: Vec<CompositionPlacement>,
}

impl MediaBridge {
    pub(crate) fn runtime_snapshot(
        &self,
    ) -> Result<(Vec<SourceSnapshot>, ProgramProgress, EngineClockMapping)> {
        let event = self.execute_kind(
            &uuid::Uuid::new_v4().to_string(),
            self.engine_run_id(),
            CommandKind::QueryRuntime,
        )?;
        match event.kind {
            EventKind::RuntimeSnapshot {
                sources,
                program,
                clock,
            } => Ok((sources, program, clock)),
            EventKind::Rejected { reason } => Err(AppError::Conflict(reason)),
            EventKind::Failed { reason } => Err(AppError::Unavailable(reason)),
            EventKind::Accepted | EventKind::Applied { .. } | EventKind::Snapshot { .. } => Err(
                AppError::Internal(anyhow::anyhow!("unexpected runtime snapshot response")),
            ),
        }
    }

    pub(crate) fn add_source(
        &self,
        command_id: &str,
        expected_engine_run_id: &str,
        source: SourceSpec,
    ) -> Result<AppliedMedia> {
        applied_event(self.execute_kind(
            command_id,
            expected_engine_run_id,
            CommandKind::AddSource { source },
        )?)
    }

    pub(crate) fn remove_source(
        &self,
        command_id: &str,
        expected_engine_run_id: &str,
        source_id: SourceId,
    ) -> Result<AppliedMedia> {
        applied_event(self.execute_kind(
            command_id,
            expected_engine_run_id,
            CommandKind::RemoveSource { source_id },
        )?)
    }

    pub(crate) fn configure_source(
        &self,
        command_id: &str,
        expected_engine_run_id: &str,
        source: SourceSpec,
    ) -> Result<AppliedMedia> {
        applied_event(self.execute_kind(
            command_id,
            expected_engine_run_id,
            CommandKind::ConfigureSource { source },
        )?)
    }

    pub(crate) fn prepare_composition(
        &self,
        request: &CompositionRequest<'_>,
    ) -> Result<AppliedMedia> {
        applied_event(self.execute_kind(
            request.command_id,
            request.expected_engine_run_id,
            CommandKind::PrepareComposition {
                revision: revision(request.revision)?,
                placements: request.placements.clone(),
            },
        )?)
    }

    pub(crate) fn take_composition(&self, request: CompositionRequest<'_>) -> Result<AppliedMedia> {
        applied_event(
            self.execute_kind(
                request.command_id,
                request.expected_engine_run_id,
                CommandKind::TakePrepared {
                    revision: revision(request.revision)?,
                    expected_program_revision: request
                        .expected_program_revision
                        .map(revision)
                        .transpose()?,
                },
            )?,
        )
    }
}

fn revision(value: &str) -> Result<ProgramRevision> {
    ProgramRevision::new(value).map_err(|error| AppError::Invalid(error.to_string()))
}
