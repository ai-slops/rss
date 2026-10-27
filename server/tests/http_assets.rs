pub mod common;

use base64::{Engine, engine::general_purpose::STANDARD};
use common::Harness;
use serde_json::json;
use std::io::Cursor;

#[tokio::test]
async fn authorized_image_upload_is_decoded_and_persisted_by_stable_id() {
    let harness = Harness::new();
    let cookie = harness.login().await;
    let before = harness.snapshot(&cookie).await;
    let mut image = Cursor::new(Vec::new());
    image::DynamicImage::new_rgba8(2, 2)
        .write_to(&mut image, image::ImageFormat::Png)
        .expect("encode fixture");
    let bytes = image.into_inner();
    let response = harness.command(&cookie, json!({
        "id": "asset-upload", "kind": "asset.upload", "targetId": before["project"]["id"],
        "expectedRevision": before["project"]["revision"], "expectedEngineRunId": before["engineRunId"],
        "payload": {"name": "Break image", "mime": "image/png", "dataBase64": STANDARD.encode(&bytes)}
    })).await;
    assert_eq!(response["status"], "applied", "{response}");
    let after = harness.snapshot(&cookie).await;
    let asset = &after["project"]["assets"][0];
    assert_eq!(asset["name"], "Break image");
    assert_eq!(asset["version"], "1");
    let path = asset["path"].as_str().expect("internal asset path");
    assert_eq!(std::fs::read(path).expect("saved pixels"), bytes);
    assert!(!path.contains("Break image"));
}

#[tokio::test]
async fn corrupt_image_upload_does_not_create_asset() {
    let harness = Harness::new();
    let cookie = harness.login().await;
    let before = harness.snapshot(&cookie).await;
    let response = harness.command(&cookie, json!({
        "id": "corrupt-upload", "kind": "asset.upload", "targetId": before["project"]["id"],
        "expectedRevision": before["project"]["revision"], "expectedEngineRunId": before["engineRunId"],
        "payload": {"name": "Not an image", "mime": "image/png", "dataBase64": STANDARD.encode(b"not image pixels")}
    })).await;
    assert_eq!(response["status"], "rejected", "{response}");
    let after = harness.snapshot(&cookie).await;
    assert!(
        after["project"]["assets"]
            .as_array()
            .expect("assets")
            .is_empty()
    );
}
