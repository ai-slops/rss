use std::{
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::Value;
use tokio::sync::{broadcast, mpsc as tokio_mpsc, oneshot};

use crate::{
    command::{Command, Receipt},
    error::{AppError, Result},
    media_bridge::MediaBridge,
    model::{AppliedScene, OutputStatus, Project, Recording, Scene},
    snapshot::Snapshot,
    store::{Event, Store},
};

#[derive(Clone)]
pub struct Coordinator {
    sender: tokio_mpsc::Sender<Request>,
    events: broadcast::Sender<()>,
}

enum Request {
    Snapshot(oneshot::Sender<Result<Snapshot>>),
    Command(Box<Command>, oneshot::Sender<Result<Value>>),
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
    pub(crate) state_directory: PathBuf,
    pub(crate) events: broadcast::Sender<()>,
}

pub(crate) struct Applied {
    pub(crate) revision: Option<String>,
    pub(crate) at_ns: String,
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
                if startup_sender.send(Ok(())).is_err() {
                    return;
                }
                while let Some(request) = receiver.blocking_recv() {
                    match request {
                        Request::Snapshot(reply) => {
                            let _ = reply.send(owner.snapshot());
                        }
                        Request::Command(command, reply) => {
                            let _ = reply.send(owner.command(*command));
                        }
                    }
                }
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

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.events.subscribe()
    }
}

impl Owner {
    fn command(&mut self, command: Command) -> Result<Value> {
        if let Some(receipt) = self
            .store
            .command(&command.id)
            .map_err(AppError::Internal)?
        {
            return Ok(receipt);
        }
        let accepted_at = now_ns();
        let result = self.apply(&command);
        let (status, applied_at, revision, reason) = match result {
            Ok(applied) => ("applied", Some(applied.at_ns), applied.revision, None),
            Err(AppError::Invalid(reason) | AppError::Conflict(reason)) => {
                ("rejected", None, None, Some(reason))
            }
            Err(AppError::Unavailable(reason)) => ("failed", None, None, Some(reason)),
            Err(AppError::Unauthorized) => {
                ("rejected", None, None, Some("Unauthorized command.".into()))
            }
            Err(AppError::Internal(_)) => (
                "failed",
                None,
                None,
                Some("The command failed; previous state is retained.".into()),
            ),
        };
        let receipt = Receipt {
            command_id: command.id.clone(),
            status,
            accepted_at,
            applied_at: applied_at.clone(),
            actual_revision: revision,
            reason,
        };
        let value =
            serde_json::to_value(receipt).map_err(|error| AppError::Internal(error.into()))?;
        self.store
            .receipt(&command.id, &value)
            .map_err(AppError::Internal)?;
        self.append_event(status, applied_at.unwrap_or_else(now_ns), value.clone())?;
        Ok(value)
    }

    fn append_event(&self, kind: &str, at: String, data: Value) -> Result<()> {
        self.store
            .append(&Event {
                cursor: String::new(),
                engine_run_id: self.media.engine_run_id().to_owned(),
                occurred_at_ns: at,
                kind: format!("command.{kind}"),
                data,
            })
            .map_err(AppError::Internal)?;
        let _ = self.events.send(());
        Ok(())
    }
}

pub(crate) fn now_ns() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
        .to_string()
}
