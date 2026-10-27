use crate::plugins::{EDITOR_PLUGIN_ID, EXTERNAL_EVENT_PLUGIN_ID};

pub(crate) struct PluginRegistration {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) storage_namespace: &'static str,
    pub(crate) commands: &'static [PluginCommandRegistration],
    pub(crate) panel: Option<PluginPanelRegistration>,
}

pub(crate) struct PluginCommandRegistration {
    pub(crate) name: &'static str,
    pub(crate) text: PluginText,
}

#[derive(Clone, Copy)]
pub(crate) enum PluginText {
    Required,
    Omitted,
}

pub(crate) struct PluginPanelRegistration {
    pub(crate) kind: &'static str,
    pub(crate) title: &'static str,
}

const EDITOR_COMMANDS: &[PluginCommandRegistration] = &[
    PluginCommandRegistration {
        name: "session.start",
        text: PluginText::Required,
    },
    PluginCommandRegistration {
        name: "marker.add",
        text: PluginText::Required,
    },
    PluginCommandRegistration {
        name: "session.export",
        text: PluginText::Omitted,
    },
    PluginCommandRegistration {
        name: "session.stop",
        text: PluginText::Omitted,
    },
];

const PANEL_COMMANDS: &[PluginCommandRegistration] = &[PluginCommandRegistration {
    name: "event.record",
    text: PluginText::Required,
}];

const REGISTRATIONS: &[PluginRegistration] = &[
    PluginRegistration {
        id: EDITOR_PLUGIN_ID,
        name: "Editor session export",
        storage_namespace: EDITOR_PLUGIN_ID,
        commands: EDITOR_COMMANDS,
        panel: Some(PluginPanelRegistration {
            kind: "editorSession",
            title: "Editor session",
        }),
    },
    PluginRegistration {
        id: EXTERNAL_EVENT_PLUGIN_ID,
        name: "External event panel",
        storage_namespace: EXTERNAL_EVENT_PLUGIN_ID,
        commands: PANEL_COMMANDS,
        panel: Some(PluginPanelRegistration {
            kind: "externalEvent",
            title: "External event",
        }),
    },
];

pub(crate) const fn registrations() -> &'static [PluginRegistration] {
    REGISTRATIONS
}
