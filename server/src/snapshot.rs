use serde::Serialize;

use crate::model::{Asset, Canvas, Id, Output, Recording, Scene, Source, TrackKind};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub cursor: String,
    pub server_time: String,
    pub engine_run_id: String,
    pub project: ProjectSnapshot,
    pub preview: PreviewSnapshot,
    pub program: ProgramSnapshot,
    pub output_statuses: Vec<OutputStatusSnapshot>,
    pub recordings: Vec<Recording>,
    pub plugins: Vec<PluginSnapshot>,
    pub storage: StorageSnapshot,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSnapshot {
    pub id: Id,
    pub name: String,
    pub revision: String,
    pub canvas: Canvas,
    pub scenes: Vec<Scene>,
    pub sources: Vec<SourceSnapshot>,
    pub audio_routes: Vec<AudioRouteSnapshot>,
    pub holdback_ms: i64,
    pub outputs: Vec<Output>,
    pub assets: Vec<Asset>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSnapshot {
    pub id: Id,
    pub name: String,
    pub enabled: bool,
    pub adapter: crate::model::SourceAdapter,
    pub tracks: Vec<TrackSnapshot>,
    pub record_on_connect: bool,
    pub dependency_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackSnapshot {
    pub id: Id,
    pub kind: TrackKind,
    pub correction_ms: i64,
    pub connection: &'static str,
    pub format: &'static str,
    pub buffer_ms: u32,
    pub late_frames: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioRouteSnapshot {
    pub track_id: Id,
    pub gain_db: f64,
    pub muted: bool,
    pub routed: bool,
    pub input_peak_db: f64,
    pub output_peak_db: f64,
    pub clipping: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewSnapshot {
    pub scene_id: Id,
    pub revision: String,
    pub rendered_revision: Option<String>,
    pub visible_revision: Option<String>,
    pub media_state: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramSnapshot {
    pub scene_id: Id,
    pub scene_revision: String,
    pub program_revision: String,
    pub applied_at_ns: String,
    pub health: &'static str,
    pub previous_scene_id: Option<Id>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputStatusSnapshot {
    pub output_id: Id,
    pub desired: &'static str,
    pub actual: &'static str,
    pub last_media_at_ns: Option<String>,
    pub error: Option<String>,
    pub platform_status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct PluginSnapshot {
    pub id: String,
    pub name: String,
    pub available: bool,
    pub enabled: bool,
    pub status: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageSnapshot {
    pub free_bytes: String,
    pub total_bytes: String,
    pub writable: bool,
}

pub fn source_snapshot(source: &Source, scenes: &[Scene]) -> SourceSnapshot {
    let dependency_count = source
        .tracks
        .iter()
        .map(|track| {
            scenes
                .iter()
                .flat_map(|scene| &scene.items)
                .filter(|item| match &item.content {
                    crate::model::Content::Track { track_id } => track_id == &track.id,
                    crate::model::Content::Solid { .. }
                    | crate::model::Content::Text { .. }
                    | crate::model::Content::Image { .. } => false,
                })
                .count()
        })
        .sum();
    SourceSnapshot {
        id: source.id.clone(),
        name: source.name.clone(),
        enabled: source.enabled,
        adapter: source.adapter.clone(),
        tracks: source
            .tracks
            .iter()
            .map(|track| TrackSnapshot {
                id: track.id.clone(),
                kind: track.kind.clone(),
                correction_ms: track.correction_ms,
                connection: "missing",
                format: "Media adapter unavailable",
                buffer_ms: 0,
                late_frames: 0,
            })
            .collect(),
        record_on_connect: source.record_on_connect,
        dependency_count,
    }
}
