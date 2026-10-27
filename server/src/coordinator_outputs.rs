use live_production_media::output::{OutputState, RecordingMapping, RecordingRecovery};

use crate::{
    command::Command,
    coordinator::{Applied, Owner, now_ns},
    error::{AppError, Result},
    model::{ActualState, DesiredState, Id, OutputAdapter, Recording},
};

impl Owner {
    pub(crate) fn shutdown_outputs(&self) -> Result<()> {
        self.outputs
            .shutdown()
            .map_err(|error| AppError::Unavailable(error.to_string()))
    }

    pub(crate) fn provision_destination(&mut self, id: &str, url: &str) -> Result<()> {
        let destination_id = live_production_media::output::OutputId::new(id)
            .map_err(|error| AppError::Invalid(error.to_string()))?;
        live_production_media::output::DestinationUrl::new(url)
            .map_err(|error| AppError::Invalid(error.to_string()))?;
        self.store
            .destination_put(destination_id.as_str(), url)
            .map_err(AppError::Internal)
    }

    pub(crate) fn start_output(
        &mut self,
        command: &Command,
        confirm_public: bool,
    ) -> Result<Applied> {
        let output = self.output(&command.target_id)?;
        if matches!(output.adapter, OutputAdapter::Rtmps { .. }) && !confirm_public {
            return Err(AppError::Conflict(
                "Confirm that sending media may start a public broadcast.".into(),
            ));
        }
        let id = media_output_id(&command.target_id)?;
        let (_, _, clock) = self.media.runtime_snapshot()?;
        self.outputs
            .start_with_clock(&id, Some(clock))
            .map_err(|error| AppError::Unavailable(error.to_string()))?;
        self.refresh_output(&command.target_id);
        Ok(applied())
    }

    pub(crate) fn stop_output(&mut self, command: &Command) -> Result<Applied> {
        self.output(&command.target_id)?;
        let id = media_output_id(&command.target_id)?;
        self.outputs
            .stop(&id)
            .map_err(|error| AppError::Unavailable(error.to_string()))?;
        self.refresh_output(&command.target_id);
        self.refresh_recordings(&command.target_id)?;
        Ok(applied())
    }

    pub(crate) fn refresh_outputs(&mut self) {
        let ids = self
            .output_statuses
            .iter()
            .map(|status| status.output_id.clone())
            .collect::<Vec<_>>();
        for id in ids {
            self.refresh_output(&id);
            let _ = self.refresh_recordings(&id);
        }
    }

    fn refresh_output(&mut self, output_id: &Id) {
        let Ok(id) = media_output_id(output_id) else {
            return;
        };
        let Ok(snapshot) = self.outputs.snapshot(&id) else {
            return;
        };
        if let Some(status) = self
            .output_statuses
            .iter_mut()
            .find(|status| status.output_id == *output_id)
        {
            status.desired = desired(snapshot.desired);
            status.actual = actual(snapshot.actual);
            status.last_media_at_ns = snapshot.last_media_at_ns.map(|value| value.to_string());
            status.error = snapshot.error;
        }
    }

    fn refresh_recordings(&mut self, output_id: &Id) -> Result<()> {
        let output = self.output(output_id)?.clone();
        if matches!(output.adapter, OutputAdapter::Rtmps { .. }) {
            return Ok(());
        }
        let id = media_output_id(output_id)?;
        let segments = self
            .outputs
            .recordings(&id)
            .map_err(|error| AppError::Unavailable(error.to_string()))?;
        self.recordings
            .retain(|recording| recording.output_id != *output_id);
        self.recordings.extend(segments.into_iter().map(|segment| {
            let file_name = segment
                .path
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap_or("unknown-fragment");
            Recording {
                id: Id(format!("recording-{}-{file_name}", segment.session_id)),
                output_id: output_id.clone(),
                name: output.name.clone(),
                path: segment.path.to_string_lossy().into_owned(),
                tap_point: tap_point(&output.adapter).into(),
                session_id: segment.session_id,
                recovery_status: recovery_status(segment.recovery).into(),
                mapping_status: mapping_status(segment.mapping).into(),
                start_ns: segment
                    .start_ns
                    .map_or_else(|| "unknown".to_owned(), |value| value.to_string()),
                end_ns: segment.end_ns.map(|value| value.to_string()),
                engine_run_id: segment.engine_run_id,
                media_start_ns: segment.media_start_ns.map(|value| value.to_string()),
                media_end_ns: segment.media_end_ns.map(|value| value.to_string()),
                media_uncertainty_ns: segment.media_uncertainty_ns.map(|value| value.to_string()),
                bytes: segment.bytes.to_string(),
                validation: if segment.decoded {
                    "decoded"
                } else {
                    "partial"
                }
                .into(),
                sha256: Some(segment.sha256),
                gaps: segment.gaps,
                provenance: segment.provenance,
                provenance_gaps: segment.provenance_gaps,
            }
        }));
        Ok(())
    }

    fn output(&self, id: &Id) -> Result<&crate::model::Output> {
        self.project
            .outputs
            .iter()
            .find(|output| output.id == *id)
            .ok_or_else(|| AppError::Invalid("Output no longer exists.".into()))
    }
}

const fn recovery_status(recovery: RecordingRecovery) -> &'static str {
    match recovery {
        RecordingRecovery::Complete => "complete",
        RecordingRecovery::Recovered => "recovered",
        RecordingRecovery::Partial => "partial",
    }
}

const fn mapping_status(mapping: RecordingMapping) -> &'static str {
    match mapping {
        RecordingMapping::Mapped => "mapped",
        RecordingMapping::Unmapped => "unmapped",
    }
}

fn media_output_id(id: &Id) -> Result<live_production_media::output::OutputId> {
    live_production_media::output::OutputId::new(&id.0)
        .map_err(|error| AppError::Invalid(error.to_string()))
}

fn applied() -> Applied {
    Applied {
        revision: None,
        at_ns: now_ns(),
        plugin_result: None,
    }
}

const fn tap_point(adapter: &OutputAdapter) -> &'static str {
    match adapter {
        OutputAdapter::ProgramRecording { .. } | OutputAdapter::Rtmps { .. } => "program",
        OutputAdapter::SourceRecording { .. } => "clean_source",
    }
}

const fn desired(state: OutputState) -> DesiredState {
    match state {
        OutputState::Running | OutputState::Starting | OutputState::Retrying => {
            DesiredState::Running
        }
        OutputState::Stopped | OutputState::Finalizing | OutputState::Failed => {
            DesiredState::Stopped
        }
    }
}

const fn actual(state: OutputState) -> ActualState {
    match state {
        OutputState::Failed => ActualState::Failed,
        OutputState::Finalizing => ActualState::Finalizing,
        OutputState::Retrying => ActualState::Retrying,
        OutputState::Running => ActualState::Running,
        OutputState::Starting => ActualState::Starting,
        OutputState::Stopped => ActualState::Stopped,
    }
}
