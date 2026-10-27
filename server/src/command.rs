use crate::model::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Command {
    pub id: String,
    pub target_id: Id,
    pub expected_revision: Option<String>,
    pub expected_engine_run_id: Option<Id>,
    #[serde(flatten)]
    pub action: Action,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum Action {
    #[serde(rename = "scene.create")]
    SceneCreate(Name),
    #[serde(rename = "scene.save")]
    SceneSave(ScenePayload),
    #[serde(rename = "scene.rename")]
    SceneRename(Name),
    #[serde(rename = "scene.duplicate")]
    SceneDuplicate(Empty),
    #[serde(rename = "scene.move")]
    SceneMove(Position),
    #[serde(rename = "scene.delete")]
    SceneDelete(Empty),
    #[serde(rename = "scene.cue")]
    SceneCue(Empty),
    #[serde(rename = "scene.take", alias = "program.take")]
    SceneTake(Take),
    #[serde(rename = "scene.item.add")]
    SceneItemAdd(AddSceneItem),
    #[serde(rename = "program.cuePrevious")]
    CuePrevious(Empty),
    #[serde(rename = "source.create")]
    SourceCreate(NewSource),
    #[serde(rename = "source.configure")]
    SourceConfigure(SourcePayload),
    #[serde(rename = "source.delete")]
    SourceDelete(Empty),
    #[serde(rename = "audio.apply")]
    AudioApply(Routes),
    #[serde(rename = "audio.set")]
    AudioSet(AudioParameters),
    #[serde(rename = "audio.gain.set")]
    AudioGainSet(GainParameter),
    #[serde(rename = "audio.mute.set")]
    AudioMuteSet(MuteParameter),
    #[serde(rename = "timing.apply")]
    TimingApply(TimingChange),
    #[serde(rename = "output.create")]
    OutputCreate(NewOutput),
    #[serde(rename = "output.start")]
    OutputStart(StartOutput),
    #[serde(rename = "output.stop")]
    OutputStop(Empty),
    #[serde(rename = "output.delete")]
    OutputDelete(Empty),
    #[serde(rename = "plugin.command")]
    Plugin(PluginCommand),
    #[serde(rename = "asset.upload")]
    AssetUpload(AssetUpload),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Empty {}
#[derive(Clone, Debug, Deserialize)]
pub struct Name {
    pub name: String,
}
#[derive(Clone, Debug, Deserialize)]
pub struct ScenePayload {
    pub scene: Scene,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Position {
    pub index: usize,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Take {
    #[serde(alias = "revision")]
    pub scene_revision: String,
    #[serde(default)]
    pub expected_program_revision: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddSceneItem {
    pub source_id: Id,
}
#[derive(Clone, Debug, Deserialize)]
pub struct NewSource {
    pub name: String,
    pub adapter: SourceAdapter,
}
#[derive(Clone, Debug, Deserialize)]
pub struct SourcePayload {
    pub source: Source,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Routes {
    pub routes: Vec<AudioRoute>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioParameters {
    pub gain_db: f64,
    pub muted: bool,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GainParameter {
    pub gain_db: f64,
}
#[derive(Clone, Debug, Deserialize)]
pub struct MuteParameter {
    pub muted: bool,
}
#[derive(Clone, Debug, Deserialize)]
pub struct NewOutput {
    pub name: String,
    pub adapter: OutputAdapter,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartOutput {
    #[serde(default)]
    pub confirm_public: bool,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginCommand {
    pub command: String,
    pub text: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetUpload {
    pub name: String,
    pub mime: String,
    pub data_base64: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub command_id: String,
    pub status: &'static str,
    pub accepted_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applied_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_result: Option<serde_json::Value>,
}
