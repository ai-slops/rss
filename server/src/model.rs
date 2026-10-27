use serde::{Deserialize, Serialize};

mod output;
mod scene;
mod source;
pub use output::*;
pub use scene::*;
pub use source::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct Id(pub String);

impl Id {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for Id {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: Id,
    pub name: String,
    pub revision: String,
    pub canvas: Canvas,
    pub scenes: Vec<Scene>,
    pub sources: Vec<Source>,
    pub audio_routes: Vec<AudioRoute>,
    pub holdback_ms: i64,
    pub outputs: Vec<Output>,
    pub assets: Vec<Asset>,
    pub plugin_settings: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub fps_numerator: u32,
    pub fps_denominator: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: Id,
    pub version: String,
    pub name: String,
    pub mime: String,
    pub path: String,
}

impl Project {
    pub fn empty() -> Self {
        Self {
            id: Id::new(),
            name: "Untitled production".into(),
            revision: "1".into(),
            canvas: Canvas {
                width: 960,
                height: 540,
                fps_numerator: 30,
                fps_denominator: 1,
            },
            scenes: vec![Scene {
                id: Id::new(),
                name: "Standby".into(),
                revision: "1".into(),
                items: vec![],
            }],
            sources: vec![],
            audio_routes: vec![],
            holdback_ms: 500,
            outputs: vec![],
            assets: vec![],
            plugin_settings: std::collections::BTreeMap::new(),
        }
    }
}
