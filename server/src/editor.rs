use serde::{Deserialize, Serialize};

use crate::{
    model::{Id, Recording, Scene},
    observation::Event,
    snapshot::{AudioRouteSnapshot, Snapshot},
    store::Store,
};

pub use crate::editor_export::EditorPackage;

const STORAGE_KEY: &str = "journal";

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorJournal {
    pub(crate) session_id: Option<String>,
    active: bool,
    pub(crate) project_id: Option<Id>,
    pub(crate) engine_run_id: Option<String>,
    started_at_ns: Option<String>,
    pub(crate) observations: Vec<EditorObservation>,
    pub(crate) markers: Vec<EditorialMarker>,
    pub(crate) gaps: Vec<EditorGap>,
    #[serde(default)]
    pub(crate) clock_anchors: Vec<EditorClockAnchor>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EditorObservation {
    pub(crate) cursor: String,
    pub(crate) occurred_at_ns: String,
    pub(crate) program_revision: String,
    pub(crate) applied_at_ns: String,
    pub(crate) scene: Scene,
    pub(crate) audio_routes: Vec<AudioRouteSnapshot>,
    pub(crate) holdback_ms: i64,
    pub(crate) track_corrections: Vec<TrackTiming>,
    pub(crate) recordings: Vec<Recording>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrackTiming {
    pub(crate) track_id: Id,
    pub(crate) correction_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EditorClockAnchor {
    pub(crate) engine_run_id: String,
    pub(crate) mapping: EditorClockMapping,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EditorClockMapping {
    pub(crate) media_time: String,
    pub(crate) wall_time: String,
    pub(crate) uncertainty: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorialMarker {
    pub(crate) label: String,
    pub(crate) at_ns: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorGap {
    pub(crate) kind: String,
    pub(crate) detail: String,
}

impl EditorJournal {
    pub fn restore(store: &Store, namespace: &str) -> anyhow::Result<Self> {
        store.plugin_get(namespace, STORAGE_KEY)?.map_or_else(
            || Ok(Self::default()),
            |value| serde_json::from_value(value).map_err(Into::into),
        )
    }

    pub fn start(
        &mut self,
        session_id: &str,
        snapshot: &Snapshot,
        store: &Store,
        namespace: &str,
    ) -> anyhow::Result<()> {
        *self = Self {
            session_id: Some(session_id.to_owned()),
            active: true,
            project_id: Some(snapshot.project.id.clone()),
            engine_run_id: Some(snapshot.engine_run_id.clone()),
            started_at_ns: Some(snapshot.server_time.clone()),
            observations: Vec::new(),
            markers: Vec::new(),
            gaps: Vec::new(),
            clock_anchors: Vec::new(),
        };
        self.capture(&snapshot.cursor, &snapshot.server_time, snapshot);
        self.persist(store, namespace)
    }

    pub fn stop(&mut self, store: &Store, namespace: &str) -> anyhow::Result<()> {
        self.active = false;
        self.persist(store, namespace)
    }

    pub fn marker(
        &mut self,
        label: &str,
        at_ns: &str,
        store: &Store,
        namespace: &str,
    ) -> anyhow::Result<()> {
        self.markers.push(EditorialMarker {
            label: label.to_owned(),
            at_ns: at_ns.to_owned(),
        });
        self.persist(store, namespace)
    }

    pub fn observe(
        &mut self,
        event: &Event,
        snapshot: &Snapshot,
        store: &Store,
        namespace: &str,
    ) -> anyhow::Result<()> {
        if self.active {
            self.capture(&event.cursor, &event.occurred_at_ns, snapshot);
            self.persist(store, namespace)?;
        }
        Ok(())
    }

    #[must_use]
    pub const fn active(&self) -> bool {
        self.active
    }

    #[must_use]
    pub const fn has_session(&self) -> bool {
        self.session_id.is_some()
    }

    #[must_use]
    pub fn status(&self) -> String {
        self.session_id.as_ref().map_or_else(
            || "No editor session".into(),
            |id| {
                format!(
                    "Session {id} {}",
                    if self.active { "active" } else { "stopped" }
                )
            },
        )
    }

    pub fn export(&self, snapshot: &Snapshot) -> anyhow::Result<EditorPackage> {
        crate::editor_export::build(self, snapshot)
    }

    fn capture(&mut self, cursor: &str, occurred_at_ns: &str, snapshot: &Snapshot) {
        self.clock_anchors.push(EditorClockAnchor {
            engine_run_id: snapshot.engine_run_id.clone(),
            mapping: EditorClockMapping {
                media_time: snapshot.engine_clock.media_time.value().to_string(),
                wall_time: snapshot.engine_clock.wall_time.value().to_string(),
                uncertainty: snapshot.engine_clock.uncertainty.value().to_string(),
            },
        });
        let scene = snapshot.program.applied_scene.clone();
        let track_corrections = snapshot
            .project
            .sources
            .iter()
            .flat_map(|source| &source.tracks)
            .map(|track| TrackTiming {
                track_id: track.id.clone(),
                correction_ms: track.correction_ms,
            })
            .collect();
        self.observations.push(EditorObservation {
            cursor: cursor.to_owned(),
            occurred_at_ns: occurred_at_ns.to_owned(),
            program_revision: snapshot.program.program_revision.clone(),
            applied_at_ns: snapshot.program.applied_at_ns.clone(),
            scene,
            audio_routes: snapshot.project.audio_routes.clone(),
            holdback_ms: snapshot.project.holdback_ms,
            track_corrections,
            recordings: snapshot.recordings.clone(),
        });
    }

    fn persist(&self, store: &Store, namespace: &str) -> anyhow::Result<()> {
        store.plugin_put(namespace, STORAGE_KEY, &serde_json::to_value(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_journal_without_clock_anchors_remains_readable() -> anyhow::Result<()> {
        let journal = serde_json::from_value::<EditorJournal>(serde_json::json!({
            "sessionId": "legacy",
            "active": false,
            "projectId": null,
            "engineRunId": null,
            "startedAtNs": null,
            "observations": [],
            "markers": [],
            "gaps": []
        }))?;
        assert!(journal.clock_anchors.is_empty());
        Ok(())
    }
}
