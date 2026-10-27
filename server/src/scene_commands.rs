use crate::{
    command::*,
    error::{AppError, Result},
    model::*,
    validation,
};

pub fn save(project: &mut Project, command: &Command, payload: &ScenePayload) -> Result<Scene> {
    let current = project
        .scenes
        .iter()
        .find(|scene| scene.id == command.target_id)
        .ok_or_else(|| AppError::Invalid("Scene no longer exists.".into()))?;
    validation::revision(command.expected_revision.as_deref(), &current.revision)?;
    if payload.scene.id != current.id {
        return Err(AppError::Invalid(
            "Scene identity cannot be changed.".into(),
        ));
    }
    validation::scene(&payload.scene, project)?;
    for (index, item) in current
        .items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.locked)
    {
        let replacement = payload
            .scene
            .items
            .iter()
            .position(|next| next.id == item.id)
            .and_then(|position| {
                payload
                    .scene
                    .items
                    .get(position)
                    .map(|next| (position, next))
            });
        let Some((position, next)) = replacement else {
            return Err(AppError::Invalid(
                "Unlock the placement before removing it.".into(),
            ));
        };
        if next.locked
            && (position != index
                || next.rect != item.rect
                || next.crop != item.crop
                || next.fit != item.fit
                || next.opacity != item.opacity)
        {
            return Err(AppError::Invalid(
                "Unlock the placement before changing its transform or order.".into(),
            ));
        }
    }
    let mut saved = payload.scene.clone();
    saved.revision = validation::next_revision(&current.revision)?;
    let slot = project
        .scenes
        .iter_mut()
        .find(|scene| scene.id == command.target_id)
        .ok_or_else(|| AppError::Invalid("Scene no longer exists.".into()))?;
    *slot = saved.clone();
    Ok(saved)
}

pub fn create(project: &mut Project, name: &str) -> Result<Scene> {
    validation::name(name)?;
    if project.scenes.len() >= 64 {
        return Err(AppError::Invalid(
            "This release supports 64 scenes per project.".into(),
        ));
    }
    let scene = Scene {
        id: Id::new(),
        name: name.into(),
        revision: "1".into(),
        items: vec![],
    };
    project.scenes.push(scene.clone());
    Ok(scene)
}

pub fn duplicate(project: &mut Project, id: &Id) -> Result<Scene> {
    let mut scene = project
        .scenes
        .iter()
        .find(|scene| &scene.id == id)
        .cloned()
        .ok_or_else(|| AppError::Invalid("Scene no longer exists.".into()))?;
    scene.id = Id::new();
    scene.name = format!("{} copy", scene.name.chars().take(90).collect::<String>());
    scene.revision = "1".into();
    for item in &mut scene.items {
        item.id = Id::new();
    }
    project.scenes.push(scene.clone());
    Ok(scene)
}

pub fn reorder(project: &mut Project, command: &Command, index: usize) -> Result<()> {
    validation::revision(command.expected_revision.as_deref(), &project.revision)?;
    if index >= project.scenes.len() {
        return Err(AppError::Invalid("Scene position is out of range.".into()));
    }
    let current = project
        .scenes
        .iter()
        .position(|scene| scene.id == command.target_id)
        .ok_or_else(|| AppError::Invalid("Scene no longer exists.".into()))?;
    let scene = project.scenes.remove(current);
    project.scenes.insert(index, scene);
    Ok(())
}

pub fn delete(project: &mut Project, command: &Command, program: &AppliedScene) -> Result<()> {
    if command.target_id == program.scene.id {
        return Err(AppError::Conflict(
            "This scene is on air. Take another scene before deleting it.".into(),
        ));
    }
    if project.scenes.len() <= 1 {
        return Err(AppError::Invalid("Keep at least one scene.".into()));
    }
    let scene = project
        .scenes
        .iter()
        .find(|scene| scene.id == command.target_id)
        .ok_or_else(|| AppError::Invalid("Scene no longer exists.".into()))?;
    validation::revision(command.expected_revision.as_deref(), &scene.revision)?;
    project.scenes.retain(|scene| scene.id != command.target_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_reuses_track_without_routing_audio() {
        let mut project = Project::empty();
        let original = project.scenes.first().expect("standby").id.clone();
        let copy = duplicate(&mut project, &original).expect("duplicate");
        assert_ne!(copy.id, original);
        assert!(project.sources.is_empty());
        assert!(project.audio_routes.is_empty());
    }

    #[test]
    fn editing_draft_does_not_mutate_pinned_program() {
        let mut project = Project::empty();
        let original = project.scenes.first().expect("standby").clone();
        let program = AppliedScene {
            scene: original.clone(),
            program_revision: "1".into(),
            engine_run_id: Id::new(),
            applied_at_ns: "0".into(),
        };
        let mut changed = original.clone();
        changed.name = "Edited off air".into();
        let payload = ScenePayload { scene: changed };
        let command = Command {
            id: "save-1".into(),
            target_id: original.id,
            expected_revision: Some("1".into()),
            expected_engine_run_id: None,
            action: Action::SceneSave(payload.clone()),
        };
        let saved = save(&mut project, &command, &payload).expect("save");
        assert_eq!(saved.revision, "2");
        assert_eq!(program.scene.name, "Standby");
        assert_eq!(program.scene.revision, "1");
    }
}
