use crate::{
    command::{Action, Command, ScenePayload},
    coordinator::{Applied, Owner, now_ns},
    error::{AppError, Result},
    model::Id,
    scene_commands, validation,
};

impl Owner {
    pub(crate) fn apply(&mut self, command: &Command) -> Result<Applied> {
        if command
            .expected_engine_run_id
            .as_ref()
            .is_some_and(|expected| expected.0 != self.media.engine_run_id())
        {
            return Err(AppError::Conflict(
                "The media engine restarted. Refresh before sending live commands.".into(),
            ));
        }
        let applied = match &command.action {
            Action::SceneCreate(payload) => {
                scene_commands::create(&mut self.project, &payload.name)?;
                self.bump_project()?;
                Applied::revision(self.project.revision.clone())
            }
            Action::SceneSave(payload) => {
                let scene = scene_commands::save(&mut self.project, command, payload)?;
                self.apply_preview_draft(&command.id, scene)?
            }
            Action::SceneRename(payload) => {
                let mut scene = self.scene(&command.target_id)?.clone();
                scene.name.clone_from(&payload.name);
                let saved =
                    scene_commands::save(&mut self.project, command, &ScenePayload { scene })?;
                self.apply_preview_draft(&command.id, saved)?
            }
            Action::SceneDuplicate(_) => {
                let source = self.scene(&command.target_id)?;
                validation::revision(command.expected_revision.as_deref(), &source.revision)?;
                scene_commands::duplicate(&mut self.project, &command.target_id)?;
                self.bump_project()?;
                Applied::revision(self.project.revision.clone())
            }
            Action::SceneMove(payload) => {
                scene_commands::reorder(&mut self.project, command, payload.index)?;
                self.bump_project()?;
                Applied::revision(self.project.revision.clone())
            }
            Action::SceneDelete(_) => {
                scene_commands::delete(&mut self.project, command, &self.program)?;
                self.bump_project()?;
                Applied::revision(self.project.revision.clone())
            }
            Action::SceneCue(_) => self.cue_scene(command)?,
            Action::SceneCopyCued(payload) => self.copy_cued_scene(command, &payload.name)?,
            Action::SceneTake(payload) => self.take_scene(command, payload)?,
            Action::SceneItemAdd(payload) => self.add_scene_item(command, &payload.source_id)?,
            Action::CuePrevious(_) => self.cue_previous(command)?,
            Action::SourceCreate(payload) => {
                self.create_source(command, &payload.name, &payload.adapter)?
            }
            Action::SourceConfigure(payload) => {
                validation::revision(command.expected_revision.as_deref(), &self.project.revision)?;
                let source_index = self
                    .project
                    .sources
                    .iter()
                    .position(|source| source.id == command.target_id)
                    .ok_or_else(|| AppError::Invalid("Source no longer exists.".into()))?;
                let current =
                    self.project.sources.get(source_index).ok_or_else(|| {
                        AppError::Internal(anyhow::anyhow!("source index missing"))
                    })?;
                if payload.source.id != current.id
                    || payload.source.tracks.len() != current.tracks.len()
                    || payload
                        .source
                        .tracks
                        .iter()
                        .zip(&current.tracks)
                        .any(|(next, old)| {
                            next.id != old.id
                                || next.kind != old.kind
                                || next.correction_ms != old.correction_ms
                        })
                {
                    return Err(AppError::Invalid(
                        "Source/track identity cannot change; apply track timing through Sync instead.".into(),
                    ));
                }
                validation::name(&payload.source.name)?;
                let credential = self
                    .store
                    .ingest_credential(&payload.source.id.0)
                    .map_err(AppError::Internal)?;
                let media = self.media.configure_source(
                    &command.id,
                    self.media.engine_run_id(),
                    crate::coordinator_sources::source_spec(&payload.source, &credential)?,
                )?;
                let source =
                    self.project.sources.get_mut(source_index).ok_or_else(|| {
                        AppError::Internal(anyhow::anyhow!("source index missing"))
                    })?;
                *source = payload.source.clone();
                self.bump_project()?;
                Applied {
                    revision: Some(self.project.revision.clone()),
                    at_ns: media.at_ns.value().to_string(),
                    plugin_result: None,
                }
            }
            Action::SourceDelete(_) => self.delete_source(command)?,
            Action::AudioApply(payload) => self.apply_audio_routes(command, &payload.routes)?,
            Action::AudioSet(payload) => {
                self.set_audio(command, Some(payload.gain_db), Some(payload.muted))?
            }
            Action::AudioGainSet(payload) => {
                self.set_audio(command, Some(payload.gain_db), None)?
            }
            Action::AudioMuteSet(payload) => self.set_audio(command, None, Some(payload.muted))?,
            Action::TimingApply(payload) => self.apply_timing(command, payload)?,
            Action::OutputCreate(payload) => self.create_output(&payload.name, &payload.adapter)?,
            Action::OutputStart(payload) => self.start_output(command, payload.confirm_public)?,
            Action::OutputStop(_) => self.stop_output(command)?,
            Action::OutputDelete(_) => self.delete_output(command)?,
            Action::Plugin(payload) => self.apply_plugin(command, payload)?,
            Action::AssetUpload(payload) => self.upload_asset(command, payload)?,
        };
        self.store.save(&self.project).map_err(AppError::Internal)?;
        Ok(applied)
    }

    pub(crate) fn scene(&self, id: &Id) -> Result<&crate::model::Scene> {
        self.project
            .scenes
            .iter()
            .find(|scene| &scene.id == id)
            .ok_or_else(|| AppError::Invalid("Scene no longer exists.".into()))
    }

    pub(crate) fn bump_project(&mut self) -> Result<()> {
        self.project.revision = validation::next_revision(&self.project.revision)?;
        Ok(())
    }

    fn apply_timing(
        &mut self,
        command: &Command,
        payload: &crate::model::TimingChange,
    ) -> Result<Applied> {
        validation::revision(command.expected_revision.as_deref(), &self.project.revision)?;
        let mut requested = HashMap::new();
        for correction in &payload.corrections {
            if !self
                .project
                .sources
                .iter()
                .flat_map(|source| &source.tracks)
                .any(|track| track.id == correction.track_id)
            {
                return Err(AppError::Invalid(
                    "Timing correction references an unknown track.".into(),
                ));
            }
            if requested
                .insert(correction.track_id.0.as_str(), correction.correction_ms)
                .is_some()
            {
                return Err(AppError::Invalid(
                    "Apply one timing correction per track.".into(),
                ));
            }
        }
        let corrections = self
            .project
            .sources
            .iter()
            .flat_map(|source| &source.tracks)
            .map(|track| {
                (
                    track.id.0.clone(),
                    requested
                        .get(track.id.0.as_str())
                        .copied()
                        .unwrap_or(track.correction_ms),
                )
            })
            .collect();
        let media = self
            .media
            .apply_track_timing(crate::media_bridge::TrackTimingRequest {
                command_id: &command.id,
                expected_engine_run_id: self.media.engine_run_id(),
                holdback_ms: payload.holdback_ms,
                corrections,
            })?;
        self.project.holdback_ms = payload.holdback_ms;
        for correction in &payload.corrections {
            if let Some(track) = self
                .project
                .sources
                .iter_mut()
                .flat_map(|source| &mut source.tracks)
                .find(|track| track.id == correction.track_id)
            {
                track.correction_ms = correction.correction_ms;
            }
        }
        self.bump_project()?;
        Ok(Applied {
            revision: Some(self.project.revision.clone()),
            at_ns: media.at_ns.value().to_string(),
            plugin_result: None,
        })
    }
}

impl Applied {
    pub(crate) fn revision(revision: String) -> Self {
        Self {
            revision: Some(revision),
            at_ns: now_ns(),
            plugin_result: None,
        }
    }
}
use std::collections::HashMap;
