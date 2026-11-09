use std::time::{Duration, Instant};

use crate::{
    coordinator::Owner,
    coordinator_outputs::media_output_id,
    error::{AppError, Result},
    model::{ActualState, DesiredState, Id, Output, OutputAdapter, OutputStatus, Project},
};

/// Persistent managed recorders retain their IDs, catalogues and files across restarts.
pub(crate) fn prepare_recordings(project: &mut Project) -> bool {
    let mut changed = false;
    for source in &mut project.sources {
        if !source.record_on_connect {
            source.record_on_connect = true;
            changed = true;
        }
        if !project.outputs.iter().any(|output| output.automatic && matches!(
            &output.adapter, OutputAdapter::SourceRecording { source_id, .. } if *source_id == source.id
        )) {
            project.outputs.push(Output {
                id: Id::new(),
                name: format!("{} archive", source.name),
                adapter: OutputAdapter::SourceRecording { source_id: source.id.clone(), segment_seconds: 60 },
                automatic: true,
            });
            changed = true;
        }
    }
    changed
}

impl Owner {
    pub(crate) fn ensure_automatic_recorders(&mut self) {
        prepare_recordings(&mut self.project);
    }

    // Runs without HTTP traffic: archiving never depends on an open browser.
    pub(crate) fn reconcile_automatic_recordings(&mut self) {
        let Ok((sources, _, clock)) = self.media.runtime_snapshot() else {
            return;
        };
        let outputs: Vec<_> = self
            .project
            .outputs
            .iter()
            .filter(|output| output.automatic)
            .cloned()
            .collect();
        for output in outputs {
            let previous = self
                .output_statuses
                .iter()
                .find(|status| status.output_id == output.id)
                .map(|status| (status.actual.clone(), status.error.clone()));
            let Ok(id) = media_output_id(&output.id) else {
                continue;
            };
            let connected = match &output.adapter {
                OutputAdapter::SourceRecording { source_id, .. } => sources
                    .iter()
                    .any(|source| source.id.as_str() == source_id.0 && source.connected),
                _ => continue,
            };
            let was_connected = self.automatic_recording_connected.contains(&output.id);
            if connected {
                self.automatic_recording_connected.insert(output.id.clone());
            } else {
                self.automatic_recording_connected.remove(&output.id);
            }
            if was_connected && !connected {
                let _ = self.outputs.stop_async(&id);
                let _ = self.refresh_recordings(&output.id);
                self.automatic_recording_retry.remove(&output.id);
                self.refresh_output(&output.id);
                continue;
            }
            let needs_start = self.outputs.snapshot(&id).map_or(true, |status| {
                matches!(
                    status.actual,
                    live_production_media::output::OutputState::Stopped
                        | live_production_media::output::OutputState::Failed
                )
            });
            if needs_start
                && self
                    .automatic_recording_retry
                    .get(&output.id)
                    .is_none_or(|last| last.elapsed() >= Duration::from_secs(5))
            {
                self.automatic_recording_retry
                    .insert(output.id.clone(), Instant::now());
                let registration = if self.outputs.snapshot(&id).is_err() {
                    self.plugins
                        .register_output(crate::plugins_factories::OutputFactoryRequest {
                            manager: &self.outputs,
                            output: &output,
                            project: &self.project,
                            store: &self.store,
                            state_directory: &self.state_directory,
                            engine_run_id: self.media.typed_engine_run_id(),
                            media: &self.media,
                        })
                } else {
                    Ok(())
                };
                let result = registration.and_then(|()| {
                    self.outputs
                        .start_with_clock(&id, Some(clock.clone()))
                        .map_err(|error| AppError::Unavailable(error.to_string()))
                });
                if !self
                    .output_statuses
                    .iter()
                    .any(|status| status.output_id == output.id)
                {
                    self.output_statuses.push(OutputStatus {
                        output_id: output.id.clone(),
                        desired: DesiredState::Running,
                        actual: ActualState::Starting,
                        last_media_at_ns: None,
                        error: None,
                        platform_status: "Source archive".into(),
                    });
                }
                if result.is_ok() {
                    self.automatic_recording_retry.remove(&output.id);
                }
                if let Err(error) = result
                    && let Some(status) = self
                        .output_statuses
                        .iter_mut()
                        .find(|status| status.output_id == output.id)
                {
                    status.actual = ActualState::Failed;
                    status.error = Some(error.to_string());
                }
            }
            self.refresh_output(&output.id);
            let current = self
                .output_statuses
                .iter()
                .find(|status| status.output_id == output.id)
                .map(|status| (status.actual.clone(), status.error.clone()));
            if current != previous {
                let _ = self.events.send(());
            }
        }
    }

    pub(crate) fn retire_automatic_recorders(&mut self, source_id: &Id) -> Result<()> {
        let ids: Vec<_> = self.project.outputs.iter().filter(|output| output.automatic && matches!(
            &output.adapter, OutputAdapter::SourceRecording { source_id: recorded, .. } if recorded == source_id
        )).map(|output| output.id.clone()).collect();
        for id in ids {
            let native = media_output_id(&id)?;
            if self.outputs.snapshot(&native).is_ok() {
                self.outputs
                    .stop(&native)
                    .map_err(|error| AppError::Unavailable(error.to_string()))?;
                self.refresh_recordings(&id)?;
                self.outputs
                    .unregister(&native)
                    .map_err(|error| AppError::Unavailable(error.to_string()))?;
            }
            self.project.outputs.retain(|output| output.id != id);
            self.output_statuses.retain(|status| status.output_id != id);
            self.automatic_recording_connected.remove(&id);
            self.automatic_recording_retry.remove(&id);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrades_old_sources_once_and_preserves_manual_outputs() {
        let mut project = Project::empty();
        let source = crate::coordinator_sources::create_source(
            &project,
            "Old camera",
            &crate::model::SourceAdapter::Synthetic {
                pattern: "ball".into(),
                frequency_hz: 440,
            },
        )
        .unwrap();
        project.sources.push(source);
        project.sources[0].record_on_connect = false;
        project.outputs.push(Output {
            id: Id::new(),
            name: "Manual Program".into(),
            automatic: false,
            adapter: OutputAdapter::ProgramRecording {
                segment_seconds: 300,
            },
        });
        assert!(prepare_recordings(&mut project));
        let id = project.outputs[1].id.clone();
        assert!(project.sources[0].record_on_connect);
        assert!(!prepare_recordings(&mut project));
        assert_eq!(project.outputs.len(), 2);
        assert_eq!(project.outputs[1].id, id);
        assert!(!project.outputs[0].automatic);
    }
}
