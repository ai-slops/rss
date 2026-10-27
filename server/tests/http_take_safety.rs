#![cfg(test)]

mod common;
use common::Harness;
use serde_json::json;

#[tokio::test]
async fn take_without_engine_identity_is_rejected_without_changing_program() {
    let app = Harness::new();
    let cookie = app.login().await;
    let initial = app.snapshot(&cookie).await;
    let response = app.command(&cookie, json!({
        "id": "unscoped-take", "kind": "scene.take", "targetId": initial["preview"]["sceneId"],
        "expectedRevision": initial["preview"]["revision"],
        "payload": {"sceneRevision": initial["preview"]["revision"], "expectedProgramRevision": initial["program"]["programRevision"]}
    })).await;
    assert_eq!(response["status"], "rejected");
    let after = app.snapshot(&cookie).await;
    assert_eq!(
        after["program"]["programRevision"],
        initial["program"]["programRevision"]
    );
}

#[tokio::test]
async fn cue_and_restore_old_program_do_not_overwrite_a_newer_saved_draft() {
    let app = Harness::new();
    let cookie = app.login().await;
    let initial = app.snapshot(&cookie).await;
    let original_id = initial["preview"]["sceneId"].clone();
    let run = initial["engineRunId"].clone();
    let mut edited = initial["project"]["scenes"][0].clone();
    edited["name"] = json!("Newer saved draft");
    let saved = app.command(&cookie, json!({
        "id": "newer-draft", "kind": "scene.save", "targetId": original_id, "expectedRevision": "1",
        "expectedEngineRunId": run, "payload": {"scene": edited}
    })).await;
    assert_eq!(saved["status"], "applied", "{saved}");
    app.command(&cookie, json!({"id": "create-break", "kind": "scene.create", "targetId": initial["project"]["id"], "expectedEngineRunId": run, "payload": {"name": "Break"}})).await;
    let current = app.snapshot(&cookie).await;
    let break_scene = &current["project"]["scenes"][1];
    let cue = app.command(&cookie, json!({"id": "cue-break", "kind": "scene.cue", "targetId": break_scene["id"], "expectedRevision": break_scene["revision"], "expectedEngineRunId": run, "payload": {}})).await;
    assert_eq!(cue["status"], "applied", "{cue}");
    let taken = app.command(&cookie, json!({
        "id": "take-break", "kind": "scene.take", "targetId": break_scene["id"], "expectedRevision": break_scene["revision"],
        "expectedEngineRunId": run, "payload": {"sceneRevision": break_scene["revision"], "expectedProgramRevision": current["program"]["programRevision"]}
    })).await;
    assert_eq!(taken["status"], "applied", "{taken}");
    let cued = app.command(&cookie, json!({"id": "cue-previous", "kind": "program.cuePrevious", "targetId": original_id, "expectedEngineRunId": run, "payload": {}})).await;
    assert_eq!(cued["status"], "applied", "{cued}");
    let old_preview = app.snapshot(&cookie).await;
    assert_eq!(old_preview["preview"]["scene"]["revision"], "1");
    assert_eq!(
        old_preview["project"]["scenes"][0]["name"],
        "Newer saved draft"
    );
    let restored = app.command(&cookie, json!({
        "id": "restore-exact-old", "kind": "scene.take", "targetId": original_id, "expectedRevision": "1",
        "expectedEngineRunId": run, "payload": {"sceneRevision": "1", "expectedProgramRevision": old_preview["program"]["programRevision"]}
    })).await;
    assert_eq!(restored["status"], "applied", "{restored}");
    let copied = app.command(&cookie, json!({"id": "copy-old", "kind": "scene.copyCued", "targetId": original_id, "expectedRevision": "1", "expectedEngineRunId": run, "payload": {"name": "Restored copy"}})).await;
    assert_eq!(copied["status"], "applied", "{copied}");
    let after = app.snapshot(&cookie).await;
    assert_eq!(after["program"]["appliedScene"]["name"], "Standby");
    assert_eq!(after["project"]["scenes"][0]["revision"], "2");
    assert_eq!(after["project"]["scenes"][0]["name"], "Newer saved draft");
    assert_ne!(after["preview"]["sceneId"], original_id);
    assert_eq!(after["preview"]["scene"]["name"], "Restored copy");
}
