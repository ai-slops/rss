use crate::{
    command::{Action, Command, ScenePayload},
    coordinator::{Applied, Owner},
    error::{AppError, Result},
    model::{
        ActualState, Content, Crop, DesiredState, Fit, Id, Output, OutputAdapter, OutputStatus,
        Rect, SceneItem, SourceAdapter, TrackKind,
    },
    scene_commands, validation,
};

impl Owner {
    pub(crate) fn add_scene_item(&mut self, command: &Command, source_id: &Id) -> Result<Applied> {
        let source = self
            .project
            .sources
            .iter()
            .find(|source| &source.id == source_id)
            .ok_or_else(|| AppError::Invalid("Source no longer exists.".into()))?;
        let track = source
            .tracks
            .iter()
            .find(|track| track.kind == TrackKind::Video)
            .ok_or_else(|| AppError::Invalid("This source has no video track.".into()))?;
        let mut scene = self.scene(&command.target_id)?.clone();
        if command.expected_revision.is_some() {
            validation::revision(command.expected_revision.as_deref(), &scene.revision)?;
        }
        scene.items.push(SceneItem {
            id: Id::new(),
            name: source.name.clone(),
            visible: true,
            locked: false,
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: f64::from(self.project.canvas.width),
                height: f64::from(self.project.canvas.height),
            },
            crop: Crop {
                left: 0.0,
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
            },
            fit: Fit::Contain,
            opacity: 1.0,
            content: Content::Track {
                track_id: track.id.clone(),
            },
        });
        let save_command = Command {
            id: command.id.clone(),
            target_id: command.target_id.clone(),
            expected_revision: Some(scene.revision.clone()),
            expected_engine_run_id: command.expected_engine_run_id.clone(),
            action: Action::SceneItemAdd(crate::command::AddSceneItem {
                source_id: source_id.clone(),
            }),
        };
        let saved =
            scene_commands::save(&mut self.project, &save_command, &ScenePayload { scene })?;
        self.apply_preview_draft(&command.id, saved)
    }

    pub(crate) fn create_source(
        &mut self,
        command: &Command,
        name: &str,
        adapter: &SourceAdapter,
    ) -> Result<Applied> {
        let source = self.plugins.create_source(&self.project, name, adapter)?;
        let credential = self
            .store
            .ingest_credential(&source.id.0)
            .map_err(AppError::Internal)?;
        let media = self.media.add_source(
            &command.id,
            self.media.engine_run_id(),
            crate::coordinator_sources::source_spec(&source, &credential)?,
        )?;
        self.media
            .apply_track_timing(crate::media_bridge::TrackTimingRequest {
                command_id: &uuid::Uuid::new_v4().to_string(),
                expected_engine_run_id: self.media.engine_run_id(),
                holdback_ms: self.project.holdback_ms,
                corrections: source
                    .tracks
                    .iter()
                    .map(|track| (track.id.0.clone(), track.correction_ms))
                    .collect(),
            })?;
        self.project.sources.push(source);
        self.ensure_automatic_recorders();
        self.bump_project()?;
        self.reconcile_automatic_recordings();
        Ok(Applied {
            revision: Some(self.project.revision.clone()),
            at_ns: media.at_ns.value().to_string(),
            plugin_result: None,
        })
    }

    pub(crate) fn delete_source(&mut self, command: &Command) -> Result<Applied> {
        if command.expected_revision.is_some() {
            validation::revision(command.expected_revision.as_deref(), &self.project.revision)?;
        }
        let source = self
            .project
            .sources
            .iter()
            .find(|source| source.id == command.target_id)
            .ok_or_else(|| AppError::Invalid("Source no longer exists.".into()))?;
        let referenced = source.tracks.iter().any(|track| self.project.scenes.iter()
            .chain([&self.program.scene, &self.preview]).chain(self.previous_program.iter().map(|program| &program.scene))
            .flat_map(|scene| &scene.items)
            .any(|item| matches!(&item.content, Content::Track { track_id } if track_id == &track.id)));
        let routed = source.tracks.iter().any(|track| {
            self.project
                .audio_routes
                .iter()
                .any(|route| route.track_id == track.id && route.routed)
        });
        let active_recording = self.project.outputs.iter().any(|output| !output.automatic && matches!(&output.adapter, OutputAdapter::SourceRecording { source_id, .. } if source_id == &source.id)
            && self.output_statuses.iter().any(|status| status.output_id == output.id && (status.desired == DesiredState::Running || status.actual == ActualState::Finalizing)));
        if referenced || routed || active_recording {
            return Err(AppError::Conflict(format!(
                "Source is still in use: visual snapshots={referenced}, live audio route={routed}, active source recorder={active_recording}. Remove draft placements, Take another layout, clear retained Preview/previous-Program dependencies, unroute audio and stop its recorder before deletion."
            )));
        }
        let source_id = live_production_media::protocol::SourceId::new(&source.id.0)
            .map_err(|error| AppError::Invalid(error.to_string()))?;
        self.retire_automatic_recorders(&command.target_id)?;
        let media = self
            .media
            .remove_source(&command.id, self.media.engine_run_id(), source_id)?;
        self.store
            .delete_ingest_credential(&command.target_id.0)
            .map_err(AppError::Internal)?;
        self.project
            .sources
            .retain(|source| source.id != command.target_id);
        self.bump_project()?;
        Ok(Applied {
            revision: Some(self.project.revision.clone()),
            at_ns: media.at_ns.value().to_string(),
            plugin_result: None,
        })
    }

    pub(crate) fn create_output(&mut self, name: &str, adapter: &OutputAdapter) -> Result<Applied> {
        validation::name(name)?;
        let output = Output {
            id: Id::new(),
            name: name.into(),
            adapter: adapter.clone(),
            automatic: false,
        };
        self.plugins
            .register_output(crate::plugins_factories::OutputFactoryRequest {
                manager: &self.outputs,
                output: &output,
                project: &self.project,
                store: &self.store,
                state_directory: &self.state_directory,
                engine_run_id: self.media.typed_engine_run_id(),
                media: &self.media,
            })?;
        self.output_statuses.push(OutputStatus {
            output_id: output.id.clone(),
            desired: DesiredState::Stopped,
            actual: ActualState::Stopped,
            last_media_at_ns: None,
            error: None,
            platform_status: "Platform status unknown".into(),
        });
        self.project.outputs.push(output);
        self.bump_project()?;
        Ok(Applied::revision(self.project.revision.clone()))
    }

    pub(crate) fn delete_output(&mut self, command: &Command) -> Result<Applied> {
        if self.output(&command.target_id)?.automatic {
            return Err(AppError::Conflict("Source recording is automatic; delete the input to retire its recorder. Saved media is retained.".into()));
        }
        let status = self
            .output_statuses
            .iter()
            .find(|status| status.output_id == command.target_id)
            .ok_or_else(|| AppError::Invalid("Output no longer exists.".into()))?;
        if status.desired == DesiredState::Running {
            return Err(AppError::Conflict(
                "Stop this output before deleting it.".into(),
            ));
        }
        self.outputs
            .unregister(&super::coordinator_outputs::media_output_id(
                &command.target_id,
            )?)
            .map_err(|error| AppError::Unavailable(error.to_string()))?;
        self.project
            .outputs
            .retain(|output| output.id != command.target_id);
        self.output_statuses
            .retain(|status| status.output_id != command.target_id);
        self.bump_project()?;
        Ok(Applied::revision(self.project.revision.clone()))
    }
}
