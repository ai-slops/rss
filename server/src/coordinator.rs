use std::{
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use live_production_media::output::OutputManager;
use serde_json::Value;
use tokio::sync::{broadcast, mpsc as tokio_mpsc, oneshot};

use crate::{
    command::Command,
    error::{AppError, Result},
    media_bridge::MediaBridge,
    model::{AppliedScene, OutputStatus, Project, Recording, Scene},
    observation::EventPage,
    plugins::PluginRegistry,
    snapshot::Snapshot,
    store::Store,
};

#[derive(Clone)]
pub struct Coordinator {
    sender: tokio_mpsc::Sender<Request>,
    events: broadcast::Sender<()>,
}

enum Request {
    Snapshot(oneshot::Sender<Result<Snapshot>>),
    Events(String, oneshot::Sender<Result<EventPage>>),
    Command(Box<Command>, oneshot::Sender<Result<Value>>),
    SourcePublishingEndpoint(String, oneshot::Sender<Result<Value>>),
    ProvisionDestination(String, String, oneshot::Sender<Result<()>>),
    Shutdown(oneshot::Sender<Result<()>>),
    Recovery(
        Box<crate::coordinator_recovery::RecoveryCommand>,
        oneshot::Sender<Result<Value>>,
    ),
}

pub(crate) struct Owner {
    pub(crate) store: Store,
    pub(crate) media: MediaBridge,
    pub(crate) project: Project,
    pub(crate) preview: Scene,
    pub(crate) program: AppliedScene,
    pub(crate) previous_program: Option<AppliedScene>,
    pub(crate) output_statuses: Vec<OutputStatus>,
    pub(crate) recordings: Vec<Recording>,
    pub(crate) outputs: OutputManager,
    pub(crate) state_directory: PathBuf,
    pub(crate) events: broadcast::Sender<()>,
    pub(crate) plugins: PluginRegistry,
    pub(crate) recovery: crate::source_recovery::SourceRecoveryManager,
    pub(crate) automatic_recording_retry:
        std::collections::HashMap<crate::model::Id, std::time::Instant>,
    pub(crate) automatic_recording_connected: std::collections::HashSet<crate::model::Id>,
}

pub(crate) struct Applied {
    pub(crate) revision: Option<String>,
    pub(crate) at_ns: String,
    pub(crate) plugin_result: Option<Value>,
}

impl Coordinator {
    pub fn spawn(database: PathBuf, media: MediaBridge) -> Result<Self> {
        let state_directory = database
            .parent()
            .map_or_else(|| PathBuf::from("."), PathBuf::from);
        let (sender, mut receiver) = tokio_mpsc::channel(64);
        let (events, _) = broadcast::channel(128);
        let (startup_sender, startup_receiver) = mpsc::sync_channel(1);
        let owner_events = events.clone();
        thread::Builder::new()
            .name("command-coordinator".into())
            .spawn(move || {
                let owner = Owner::open(database, state_directory, media, owner_events);
                let Ok(mut owner) = owner else {
                    let _ = startup_sender.send(owner.map(|_| ()));
                    return;
                };
                let runtime = match tokio::runtime::Builder::new_current_thread().enable_time().build() {
                    Ok(runtime) => runtime,
                    Err(error) => { let _ = startup_sender.send(Err(AppError::Internal(error.into()))); return; }
                };
                owner.reconcile_automatic_recordings();
                if startup_sender.send(Ok(())).is_err() {
                    return;
                }
                runtime.block_on(async {
                let mut tick = tokio::time::interval(std::time::Duration::from_millis(250));
                tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    let request = tokio::select! {
                        request = receiver.recv() => match request { Some(request) => request, None => break },
                        _ = tick.tick() => { owner.reconcile_automatic_recordings(); continue; }
                    };
                    match request {
                        Request::Snapshot(reply) => {
                            let _ = reply.send(owner.snapshot());
                        }
                        Request::Events(after, reply) => {
                            let _ = reply.send(owner.events(&after));
                        }
                        Request::Command(command, reply) => {
                            let _ = reply.send(owner.command(*command));
                        }
                        Request::SourcePublishingEndpoint(id, reply) => {
                            let _ = reply.send(owner.source_publishing_endpoint(&id));
                        }
                        Request::ProvisionDestination(id, url, reply) => {
                            let _ = reply.send(owner.provision_destination(&id, &url));
                        }
                        Request::Shutdown(reply) => {
                            let result = owner.shutdown_outputs();
                            let _ = reply.send(result);
                            break;
                        }
                        Request::Recovery(command, reply) => {
                            let _ = reply.send(owner.recovery(*command));
                        }
                    }
                }
                });
            })
            .map_err(|error| AppError::Internal(error.into()))?;
        startup_receiver
            .recv()
            .map_err(|error| AppError::Internal(error.into()))??;
        Ok(Self { sender, events })
    }

    pub async fn snapshot(&self) -> Result<Snapshot> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(Request::Snapshot(reply))
            .await
            .map_err(|_| AppError::Unavailable("Command coordinator stopped.".into()))?;
        response
            .await
            .map_err(|_| AppError::Unavailable("Command coordinator stopped.".into()))?
    }

    pub async fn execute(&self, command: Command) -> Result<Value> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(Request::Command(Box::new(command), reply))
            .await
            .map_err(|_| AppError::Unavailable("Command coordinator stopped.".into()))?;
        response
            .await
            .map_err(|_| AppError::Unavailable("Command coordinator stopped.".into()))?
    }

    pub async fn source_publishing_endpoint(&self, id: String) -> Result<Value> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(Request::SourcePublishingEndpoint(id, reply))
            .await
            .map_err(|_| AppError::Unavailable("Command coordinator stopped.".into()))?;
        response
            .await
            .map_err(|_| AppError::Unavailable("Command coordinator stopped.".into()))?
    }

    pub async fn provision_destination(&self, id: String, url: String) -> Result<()> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(Request::ProvisionDestination(id, url, reply))
            .await
            .map_err(|_| AppError::Unavailable("Command coordinator stopped.".into()))?;
        response
            .await
            .map_err(|_| AppError::Unavailable("Command coordinator stopped.".into()))?
    }

    pub async fn events(&self, after: &str) -> Result<EventPage> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(Request::Events(after.to_owned(), reply))
            .await
            .map_err(|_| AppError::Unavailable("Command coordinator stopped.".into()))?;
        response
            .await
            .map_err(|_| AppError::Unavailable("Command coordinator stopped.".into()))?
    }

    pub async fn shutdown(&self) -> Result<()> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(Request::Shutdown(reply))
            .await
            .map_err(|_| AppError::Unavailable("Command coordinator stopped.".into()))?;
        response
            .await
            .map_err(|_| AppError::Unavailable("Command coordinator stopped.".into()))?
    }

    pub(crate) async fn recovery(
        &self,
        command: crate::coordinator_recovery::RecoveryCommand,
    ) -> Result<Value> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(Request::Recovery(Box::new(command), reply))
            .await
            .map_err(|_| AppError::Unavailable("Command coordinator stopped.".into()))?;
        response
            .await
            .map_err(|_| AppError::Unavailable("Command coordinator stopped.".into()))?
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.events.subscribe()
    }
}

pub(crate) fn now_ns() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
        .to_string()
}
