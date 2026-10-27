use crate::{
    command::{Command, Take},
    coordinator::{Applied, Owner},
    error::{AppError, Result},
    media_bridge_runtime::CompositionRequest,
    model::{AppliedScene, Scene},
    validation,
};

impl Owner {
    pub(crate) fn cue_scene(&mut self, command: &Command) -> Result<Applied> {
        let scene = self.scene(&command.target_id)?.clone();
        validation::revision(command.expected_revision.as_deref(), &scene.revision)?;
        let revision = media_revision(&scene);
        let applied = self.media.prepare_composition(&CompositionRequest {
            command_id: &command.id,
            expected_engine_run_id: self.media.engine_run_id(),
            revision: &revision,
            expected_program_revision: None,
            placements: crate::media_bridge_layout::composition_placements(
                &scene,
                &self.project.assets,
            )?,
        })?;
        self.preview = scene;
        Ok(Applied {
            revision: Some(self.preview.revision.clone()),
            at_ns: applied.at_ns.value().to_string(),
            plugin_result: None,
        })
    }

    pub(crate) fn take_scene(&mut self, command: &Command, payload: &Take) -> Result<Applied> {
        let scene = self.scene(&command.target_id)?.clone();
        validation::revision(Some(&payload.scene_revision), &scene.revision)?;
        let expected_program = payload
            .expected_program_revision
            .as_deref()
            .or(command.expected_revision.as_deref());
        validation::revision(expected_program, &self.program.program_revision)?;
        let revision = media_revision(&scene);
        let applied = self.media.take_composition(CompositionRequest {
            command_id: &command.id,
            expected_engine_run_id: self.media.engine_run_id(),
            revision: &revision,
            expected_program_revision: Some(&self.program.program_revision),
            placements: crate::media_bridge_layout::composition_placements(
                &scene,
                &self.project.assets,
            )?,
        })?;
        let actual_revision = applied
            .program_revision
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Take omitted Program revision")))?;
        self.previous_program = Some(self.program.clone());
        self.program = AppliedScene {
            scene,
            program_revision: actual_revision.clone(),
            engine_run_id: crate::model::Id(self.media.engine_run_id().to_owned()),
            applied_at_ns: applied.at_ns.value().to_string(),
        };
        Ok(Applied {
            revision: Some(actual_revision),
            at_ns: self.program.applied_at_ns.clone(),
            plugin_result: None,
        })
    }

    pub(crate) fn cue_previous(&mut self, command: &Command) -> Result<Applied> {
        let previous = self
            .previous_program
            .as_ref()
            .ok_or_else(|| {
                AppError::Invalid("There is no previous Program snapshot to cue.".into())
            })?
            .clone();
        let revision = media_revision(&previous.scene);
        let applied = self.media.prepare_composition(&CompositionRequest {
            command_id: &command.id,
            expected_engine_run_id: self.media.engine_run_id(),
            revision: &revision,
            expected_program_revision: None,
            placements: crate::media_bridge_layout::composition_placements(
                &previous.scene,
                &self.project.assets,
            )?,
        })?;
        self.preview = previous.scene;
        Ok(Applied {
            revision: Some(self.preview.revision.clone()),
            at_ns: applied.at_ns.value().to_string(),
            plugin_result: None,
        })
    }
}

pub(crate) fn media_revision(scene: &Scene) -> String {
    format!("{}.{}", scene.id.0, scene.revision)
}
