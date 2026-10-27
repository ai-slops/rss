use anyhow::Context;
use live_production::{
    command::{Action, Command, ScenePayload},
    model::*,
    scene_commands,
};

fn camera_item(track_id: Id) -> SceneItem {
    SceneItem {
        id: Id::new(),
        name: "Camera placement".into(),
        visible: true,
        locked: false,
        rect: Rect {
            x: 100.0,
            y: 50.0,
            width: 640.0,
            height: 360.0,
        },
        crop: Crop {
            left: 0.0,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
        },
        fit: Fit::Contain,
        opacity: 1.0,
        content: Content::Track { track_id },
    }
}

fn project_with_camera() -> anyhow::Result<Project> {
    let mut project = Project::empty();
    let track_id = Id::new();
    project.sources.push(Source {
        id: Id::new(),
        name: "Camera".into(),
        enabled: true,
        adapter: SourceAdapter::Synthetic {
            pattern: "smpte".into(),
            frequency_hz: 440,
        },
        tracks: vec![Track {
            id: track_id.clone(),
            kind: TrackKind::Video,
            correction_ms: 0,
        }],
        record_on_connect: false,
    });
    project
        .scenes
        .first_mut()
        .context("standby scene missing from fixture")?
        .items
        .push(camera_item(track_id));
    Ok(project)
}

fn save_command(scene: Scene, revision: &str) -> Command {
    Command {
        id: uuid::Uuid::new_v4().to_string(),
        target_id: scene.id.clone(),
        expected_revision: Some(revision.into()),
        expected_engine_run_id: None,
        action: Action::SceneSave(ScenePayload { scene }),
    }
}

#[test]
fn duplicate_preserves_track_identity_without_duplicate_capture_or_audio() {
    let mut project = project_with_camera().expect("camera fixture");
    let original = project.scenes.first().expect("scene").clone();
    let copied = scene_commands::duplicate(&mut project, &original.id).expect("duplicate");
    let original_item = original.items.first().expect("original placement");
    let copied_item = copied.items.first().expect("copied placement");
    assert_ne!(copied_item.id, original_item.id);
    match (&original_item.content, &copied_item.content) {
        (Content::Track { track_id: a }, Content::Track { track_id: b }) => assert_eq!(a, b),
        _ => panic!("camera content must remain a track"),
    }
    assert_eq!(project.sources.len(), 1);
    assert!(project.audio_routes.is_empty());
}

#[test]
fn a_stale_scene_save_preserves_newer_draft() {
    let mut project = project_with_camera().expect("camera fixture");
    let original = project.scenes.first().expect("scene").clone();
    let mut first = original.clone();
    first.name = "Operator A saved".into();
    let payload = ScenePayload {
        scene: first.clone(),
    };
    scene_commands::save(&mut project, &save_command(first, "1"), &payload).expect("first save");
    let mut stale = original;
    stale.name = "Operator B stale text".into();
    let result = scene_commands::save(
        &mut project,
        &save_command(stale.clone(), "1"),
        &ScenePayload { scene: stale },
    );
    assert!(result.is_err());
    assert_eq!(
        project.scenes.first().expect("saved scene").name,
        "Operator A saved"
    );
}

#[test]
fn locked_placement_rejects_removal_but_allows_explicit_visibility() {
    let mut project = project_with_camera().expect("camera fixture");
    project
        .scenes
        .first_mut()
        .expect("scene")
        .items
        .first_mut()
        .expect("placement")
        .locked = true;
    let original = project.scenes.first().expect("scene").clone();
    let mut removed = original.clone();
    removed.items.clear();
    let result = scene_commands::save(
        &mut project,
        &save_command(removed.clone(), "1"),
        &ScenePayload { scene: removed },
    );
    assert!(result.is_err());
    let mut hidden = original;
    hidden.items.first_mut().expect("placement").visible = false;
    let result = scene_commands::save(
        &mut project,
        &save_command(hidden.clone(), "1"),
        &ScenePayload { scene: hidden },
    );
    assert!(result.is_ok());
}

#[test]
fn locked_placement_rejects_crop_changes() {
    let mut project = project_with_camera().expect("camera fixture");
    let current = project.scenes.first_mut().expect("scene");
    current.items.first_mut().expect("placement").locked = true;
    let mut changed = current.clone();
    changed.items.first_mut().expect("placement").crop.left = 0.1;
    let command = save_command(changed.clone(), "1");
    let result = scene_commands::save(&mut project, &command, &ScenePayload { scene: changed });
    assert!(
        result.is_err(),
        "locked crop must remain pinned until explicitly unlocked"
    );
}
