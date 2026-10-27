use live_production_media::protocol::ScenePlacement;
use num_traits::ToPrimitive;

use crate::{
    command::{Command, Take},
    coordinator::{Applied, Owner},
    error::{AppError, Result},
    media_bridge::{PrepareRequest, TakeRequest},
    model::{AppliedScene, Scene},
    validation,
};

const MEDIA_SCENE_SLOTS: usize = 8;

impl Owner {
    pub(crate) fn cue_scene(&mut self, command: &Command) -> Result<Applied> {
        let scene = self.scene(&command.target_id)?.clone();
        validation::revision(command.expected_revision.as_deref(), &scene.revision)?;
        let revision = media_revision(&scene);
        let applied = self.media.prepare(PrepareRequest {
            command_id: &command.id,
            expected_engine_run_id: self.media.engine_run_id(),
            revision: &revision,
            placements: scene_placements(&scene)?,
        })?;
        self.preview = scene;
        Ok(Applied {
            revision: Some(self.preview.revision.clone()),
            at_ns: applied.at_ns.value().to_string(),
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
        let applied = self.media.take(TakeRequest {
            command_id: &command.id,
            expected_engine_run_id: self.media.engine_run_id(),
            revision: &revision,
            expected_program_revision: Some(&self.program.program_revision),
            placements: scene_placements(&scene)?,
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
        let applied = self.media.prepare(PrepareRequest {
            command_id: &command.id,
            expected_engine_run_id: self.media.engine_run_id(),
            revision: &revision,
            placements: scene_placements(&previous.scene)?,
        })?;
        self.preview = previous.scene;
        Ok(Applied {
            revision: Some(self.preview.revision.clone()),
            at_ns: applied.at_ns.value().to_string(),
        })
    }
}

pub(crate) fn media_revision(scene: &Scene) -> String {
    format!("{}.{}", scene.id.0, scene.revision)
}

pub(crate) fn scene_placements(scene: &Scene) -> Result<Vec<ScenePlacement>> {
    if scene.items.len() > MEDIA_SCENE_SLOTS {
        return Err(AppError::Unavailable(format!(
            "The native compositor currently supports {MEDIA_SCENE_SLOTS} scene placements."
        )));
    }
    let mut placements = Vec::with_capacity(MEDIA_SCENE_SLOTS);
    for slot in 0..MEDIA_SCENE_SLOTS {
        placements.push(ScenePlacement {
            slot: u16::try_from(slot).map_err(|error| AppError::Internal(error.into()))?,
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            opacity_milli: 0,
        });
    }
    for (slot, item) in scene.items.iter().enumerate() {
        let placement = placements
            .get_mut(slot)
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("scene slot missing")))?;
        placement.x = coordinate(item.rect.x)?;
        placement.y = coordinate(item.rect.y)?;
        placement.width = coordinate(item.rect.width)?;
        placement.height = coordinate(item.rect.height)?;
        placement.opacity_milli = if item.visible {
            (item.opacity * 1_000.0)
                .round()
                .to_u16()
                .ok_or_else(|| AppError::Invalid("Scene opacity is out of range.".into()))?
        } else {
            0
        };
    }
    Ok(placements)
}

fn coordinate(value: f64) -> Result<i32> {
    value
        .round()
        .to_i32()
        .ok_or_else(|| AppError::Invalid("Scene geometry is out of range.".into()))
}
