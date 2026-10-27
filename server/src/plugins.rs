use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    command::PluginCommand,
    editor::EditorJournal,
    observation::Event,
    plugins_registration::{PluginText, registrations},
    snapshot::{PluginCommandSnapshot, PluginPanelSnapshot, PluginSnapshot, Snapshot},
    store::Store,
};

pub use crate::plugins_factories::{
    OUTPUT_FACTORY_KINDS, OutputFactoryKind, SOURCE_FACTORY_KINDS, SourceFactoryKind,
};

pub const EDITOR_PLUGIN_ID: &str = "editor-session";
pub const EXTERNAL_EVENT_PLUGIN_ID: &str = "external-event-panel";
const PANEL_STORAGE_KEY: &str = "state";

pub struct PluginRegistry {
    editor: EditorJournal,
    panel: ExternalEventPanel,
    editor_error: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExternalEventPanel {
    event_count: u64,
    latest: Option<String>,
}

impl PluginRegistry {
    pub fn trusted(store: &Store) -> anyhow::Result<Self> {
        let editor = EditorJournal::restore(store, EDITOR_PLUGIN_ID)?;
        let panel = store
            .plugin_get(EXTERNAL_EVENT_PLUGIN_ID, PANEL_STORAGE_KEY)?
            .map_or_else(
                || Ok(ExternalEventPanel::default()),
                |value| serde_json::from_value(value).map_err(anyhow::Error::from),
            )?;
        Ok(Self {
            editor,
            panel,
            editor_error: None,
        })
    }

    pub(crate) fn create_source(
        &self,
        project: &crate::model::Project,
        name: &str,
        adapter: &crate::model::SourceAdapter,
    ) -> crate::error::Result<crate::model::Source> {
        crate::plugins_factories::create(crate::plugins_factories::SourceFactoryRequest {
            project,
            name,
            adapter,
        })
    }

    pub(crate) fn register_output(
        &self,
        request: crate::plugins_factories::OutputFactoryRequest<'_>,
    ) -> crate::error::Result<()> {
        crate::plugins_factories::register(request)
    }

    #[must_use]
    pub fn snapshots(&self) -> Vec<PluginSnapshot> {
        registrations()
            .iter()
            .map(|registration| PluginSnapshot {
                id: registration.id.into(),
                name: registration.name.into(),
                storage_namespace: registration.storage_namespace,
                available: true,
                enabled: match registration.id {
                    EDITOR_PLUGIN_ID => self.editor.active(),
                    EXTERNAL_EVENT_PLUGIN_ID => true,
                    _ => false,
                },
                status: match registration.id {
                    EDITOR_PLUGIN_ID => self
                        .editor_error
                        .clone()
                        .unwrap_or_else(|| self.editor.status()),
                    EXTERNAL_EVENT_PLUGIN_ID => format!("{} events", self.panel.event_count),
                    _ => "Unavailable".into(),
                },
                commands: registration
                    .commands
                    .iter()
                    .map(|command| PluginCommandSnapshot {
                        name: command.name,
                        text: match command.text {
                            PluginText::Required => "required",
                            PluginText::Omitted => "omitted",
                        },
                    })
                    .collect(),
                panel: registration
                    .panel
                    .as_ref()
                    .map(|panel| PluginPanelSnapshot {
                        kind: panel.kind,
                        title: panel.title,
                    }),
            })
            .collect()
    }

    pub fn execute(
        &mut self,
        plugin_id: &str,
        command: &PluginCommand,
        snapshot: &Snapshot,
        store: &Store,
    ) -> Result<Value, PluginError> {
        match plugin_id {
            EDITOR_PLUGIN_ID => self.execute_editor(command, snapshot, store),
            EXTERNAL_EVENT_PLUGIN_ID => self.execute_panel(command, store),
            _ => Err(PluginError::Invalid(
                "The requested plugin is unavailable.".into(),
            )),
        }
    }

    pub fn observe(&mut self, event: &Event, snapshot: &Snapshot, store: &Store) {
        if let Err(error) = self
            .editor
            .observe(event, snapshot, store, EDITOR_PLUGIN_ID)
        {
            self.editor_error = Some(format!("Journal observation failed: {error}"));
        }
    }

    fn execute_editor(
        &mut self,
        command: &PluginCommand,
        snapshot: &Snapshot,
        store: &Store,
    ) -> Result<Value, PluginError> {
        match command.command.as_str() {
            "session.start" => {
                let session_id = required_text(command)?;
                let mut editor = self.editor.clone();
                editor
                    .start(session_id, snapshot, store, EDITOR_PLUGIN_ID)
                    .map_err(PluginError::Internal)?;
                self.editor = editor;
                self.editor_error = None;
                Ok(json!({"sessionId": session_id, "active": true}))
            }
            "marker.add" => {
                if !self.editor.active() {
                    return Err(PluginError::Invalid(
                        "Start an editor session before adding markers.".into(),
                    ));
                }
                let label = required_text(command)?;
                let mut editor = self.editor.clone();
                editor
                    .marker(label, &snapshot.server_time, store, EDITOR_PLUGIN_ID)
                    .map_err(PluginError::Internal)?;
                self.editor = editor;
                Ok(json!({"markerAdded": true}))
            }
            "session.export" => {
                if !self.editor.has_session() {
                    return Err(PluginError::Invalid(
                        "Start an editor session before export.".into(),
                    ));
                }
                serde_json::to_value(
                    self.editor
                        .export(snapshot)
                        .map_err(PluginError::Internal)?,
                )
                .map_err(anyhow::Error::from)
                .map_err(PluginError::Internal)
            }
            "session.stop" => {
                let mut editor = self.editor.clone();
                editor
                    .stop(store, EDITOR_PLUGIN_ID)
                    .map_err(PluginError::Internal)?;
                self.editor = editor;
                Ok(json!({"active": false}))
            }
            _ => Err(PluginError::Invalid(
                "Unknown editor plugin command.".into(),
            )),
        }
    }

    fn execute_panel(
        &mut self,
        command: &PluginCommand,
        store: &Store,
    ) -> Result<Value, PluginError> {
        match command.command.as_str() {
            "event.record" => {
                let text = required_text(command)?;
                let panel = ExternalEventPanel {
                    event_count: self.panel.event_count.saturating_add(1),
                    latest: Some(text.to_owned()),
                };
                let state = serde_json::to_value(&panel)
                    .map_err(anyhow::Error::from)
                    .map_err(PluginError::Internal)?;
                store
                    .plugin_put(EXTERNAL_EVENT_PLUGIN_ID, PANEL_STORAGE_KEY, &state)
                    .map_err(PluginError::Internal)?;
                self.panel = panel;
                Ok(json!({"eventCount": self.panel.event_count}))
            }
            _ => Err(PluginError::Invalid(
                "Unknown external event panel command.".into(),
            )),
        }
    }
}

fn required_text(command: &PluginCommand) -> Result<&str, PluginError> {
    command
        .text
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| PluginError::Invalid("This plugin command requires non-empty text.".into()))
}
