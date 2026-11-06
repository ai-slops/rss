use std::path::Path;

use live_production_media::{output::OutputManager, protocol::EngineRunId};

use crate::{
    error::{AppError, Result},
    model::{Output, OutputAdapter, Project, Source, SourceAdapter},
    store::Store,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceFactoryKind {
    Synthetic,
    Srt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFactoryKind {
    ProgramRecording,
    SourceRecording,
    Rtmps,
}

pub const SOURCE_FACTORY_KINDS: &[SourceFactoryKind] =
    &[SourceFactoryKind::Synthetic, SourceFactoryKind::Srt];
pub const OUTPUT_FACTORY_KINDS: &[OutputFactoryKind] = &[
    OutputFactoryKind::ProgramRecording,
    OutputFactoryKind::SourceRecording,
    OutputFactoryKind::Rtmps,
];

pub(crate) struct SourceFactoryRequest<'a> {
    pub(crate) project: &'a Project,
    pub(crate) name: &'a str,
    pub(crate) adapter: &'a SourceAdapter,
}

pub(crate) struct OutputFactoryRequest<'a> {
    pub(crate) manager: &'a OutputManager,
    pub(crate) output: &'a Output,
    pub(crate) project: &'a Project,
    pub(crate) store: &'a Store,
    pub(crate) state_directory: &'a Path,
    pub(crate) engine_run_id: &'a EngineRunId,
    pub(crate) media: &'a crate::media_bridge::MediaBridge,
}

type SourceFactory = for<'a> fn(SourceFactoryRequest<'a>) -> Result<Source>;
type OutputFactory = for<'a> fn(OutputFactoryRequest<'a>) -> Result<()>;

struct SourceRegistration {
    kind: SourceFactoryKind,
    create: SourceFactory,
}

struct OutputRegistration {
    kind: OutputFactoryKind,
    register: OutputFactory,
}

const SOURCE_FACTORIES: &[SourceRegistration] = &[
    SourceRegistration {
        kind: SourceFactoryKind::Synthetic,
        create: create_source,
    },
    SourceRegistration {
        kind: SourceFactoryKind::Srt,
        create: create_source,
    },
];

const OUTPUT_FACTORIES: &[OutputRegistration] = &[
    OutputRegistration {
        kind: OutputFactoryKind::ProgramRecording,
        register: register_output,
    },
    OutputRegistration {
        kind: OutputFactoryKind::SourceRecording,
        register: register_output,
    },
    OutputRegistration {
        kind: OutputFactoryKind::Rtmps,
        register: register_output,
    },
];

pub(crate) fn create(request: SourceFactoryRequest<'_>) -> Result<Source> {
    let kind = match request.adapter {
        SourceAdapter::Synthetic { .. } => SourceFactoryKind::Synthetic,
        SourceAdapter::Srt { .. } => SourceFactoryKind::Srt,
    };
    let registration = SOURCE_FACTORIES
        .iter()
        .find(|registration| registration.kind == kind)
        .ok_or_else(|| AppError::Unavailable("Source factory is not registered.".into()))?;
    (registration.create)(request)
}

pub(crate) fn register(request: OutputFactoryRequest<'_>) -> Result<()> {
    let kind = match request.output.adapter {
        OutputAdapter::ProgramRecording { .. } => OutputFactoryKind::ProgramRecording,
        OutputAdapter::SourceRecording { .. } => OutputFactoryKind::SourceRecording,
        OutputAdapter::Rtmps { .. } => OutputFactoryKind::Rtmps,
    };
    let registration = OUTPUT_FACTORIES
        .iter()
        .find(|registration| registration.kind == kind)
        .ok_or_else(|| AppError::Unavailable("Output factory is not registered.".into()))?;
    (registration.register)(request)
}

fn create_source(request: SourceFactoryRequest<'_>) -> Result<Source> {
    crate::coordinator_sources::create_source(request.project, request.name, request.adapter)
}

fn register_output(request: OutputFactoryRequest<'_>) -> Result<()> {
    crate::output::register(request)
}
