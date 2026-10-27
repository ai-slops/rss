use super::Id;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    pub id: Id,
    pub name: String,
    pub revision: String,
    pub items: Vec<SceneItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneItem {
    pub id: Id,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub rect: Rect,
    pub crop: Crop,
    pub fit: Fit,
    pub opacity: f64,
    pub content: Content,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Crop {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Fit {
    Contain,
    Cover,
    Stretch,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Content {
    Track {
        #[serde(rename = "trackId")]
        track_id: Id,
    },
    Solid {
        color: String,
    },
    Text {
        text: String,
        color: String,
        #[serde(rename = "fontSize")]
        font_size: u32,
    },
    Image {
        #[serde(rename = "assetId")]
        asset_id: Id,
        version: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedScene {
    pub scene: Scene,
    pub program_revision: String,
    pub engine_run_id: Id,
    pub applied_at_ns: String,
}
