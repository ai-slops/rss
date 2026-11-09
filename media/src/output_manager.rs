use std::{
    collections::HashMap,
    sync::{Arc, Mutex, mpsc},
    thread::JoinHandle,
};

use crate::protocol::EngineClockMapping;
use crate::{
    output::{OutputDefinition, OutputError, OutputId, OutputSnapshot, RecordingSegment},
    output_owner::{OwnerCommand, SharedOutput, SpawnedOwner, spawn_owner},
};

pub struct OutputManager {
    owners: Mutex<HashMap<OutputId, OutputHandle>>,
}

struct OutputHandle {
    sender: mpsc::Sender<OwnerCommand>,
    shared: Arc<Mutex<SharedOutput>>,
    worker: Option<JoinHandle<()>>,
}

impl OutputManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            owners: Mutex::new(HashMap::new()),
        }
    }

    pub fn register(&self, definition: OutputDefinition) -> Result<(), OutputError> {
        let id = definition.id().clone();
        let mut owners = self.owners.lock().map_err(|_| OutputError::OwnerStopped)?;
        if owners.contains_key(&id) {
            return Err(OutputError::AlreadyRegistered);
        }
        let SpawnedOwner {
            sender,
            shared,
            worker,
        } = spawn_owner(definition)?;
        owners.insert(
            id,
            OutputHandle {
                sender,
                shared,
                worker: Some(worker),
            },
        );
        drop(owners);
        Ok(())
    }

    pub fn start(&self, id: &OutputId) -> Result<(), OutputError> {
        self.start_with_clock(id, None)
    }

    pub fn start_with_clock(
        &self,
        id: &OutputId,
        clock: Option<EngineClockMapping>,
    ) -> Result<(), OutputError> {
        let sender = self.sender(id)?;
        let (reply, response) = mpsc::sync_channel(1);
        sender
            .send(OwnerCommand::Start { clock, reply })
            .map_err(|_| OutputError::OwnerStopped)?;
        response.recv().map_err(|_| OutputError::OwnerStopped)?
    }

    pub fn stop(&self, id: &OutputId) -> Result<(), OutputError> {
        self.request(id, OwnerCommand::stop)
    }

    pub fn snapshot(&self, id: &OutputId) -> Result<OutputSnapshot, OutputError> {
        let shared = self.shared(id)?;
        let snapshot = shared
            .lock()
            .map_err(|_| OutputError::OwnerStopped)?
            .snapshot
            .clone();
        Ok(snapshot)
    }

    pub fn recordings(&self, id: &OutputId) -> Result<Vec<RecordingSegment>, OutputError> {
        let shared = self.shared(id)?;
        let recordings = shared
            .lock()
            .map_err(|_| OutputError::OwnerStopped)?
            .recordings
            .clone();
        Ok(recordings)
    }

    pub fn unregister(&self, id: &OutputId) -> Result<(), OutputError> {
        let mut owner = self
            .owners
            .lock()
            .map_err(|_| OutputError::OwnerStopped)?
            .remove(id)
            .ok_or(OutputError::UnknownOutput)?;
        let (reply, response) = mpsc::sync_channel(1);
        let result = owner
            .sender
            .send(OwnerCommand::shutdown(reply))
            .map_err(|_| OutputError::OwnerStopped)
            .and_then(|()| response.recv().map_err(|_| OutputError::OwnerStopped)?);
        if owner
            .worker
            .take()
            .is_some_and(|worker| worker.join().is_err())
        {
            return Err(OutputError::OwnerStopped);
        }
        result
    }

    pub fn shutdown(&self) -> Result<(), OutputError> {
        let mut owners = self.owners.lock().map_err(|_| OutputError::OwnerStopped)?;
        let mut first_error = None;
        for owner in owners.values_mut() {
            if owner.worker.is_none() {
                continue;
            }
            let (reply, response) = mpsc::sync_channel(1);
            if owner.sender.send(OwnerCommand::shutdown(reply)).is_err() {
                first_error.get_or_insert(OutputError::OwnerStopped);
            } else {
                match response.recv() {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        first_error.get_or_insert(error);
                    }
                    Err(_) => {
                        first_error.get_or_insert(OutputError::OwnerStopped);
                    }
                }
            }
            if owner
                .worker
                .take()
                .is_some_and(|worker| worker.join().is_err())
            {
                first_error.get_or_insert(OutputError::OwnerStopped);
            }
        }
        drop(owners);
        first_error.map_or(Ok(()), Err)
    }

    fn request(
        &self,
        id: &OutputId,
        command: fn(mpsc::SyncSender<Result<(), OutputError>>) -> OwnerCommand,
    ) -> Result<(), OutputError> {
        let sender = self
            .owners
            .lock()
            .map_err(|_| OutputError::OwnerStopped)?
            .get(id)
            .map(|owner| owner.sender.clone())
            .ok_or(OutputError::UnknownOutput)?;
        let (reply, response) = mpsc::sync_channel(1);
        sender
            .send(command(reply))
            .map_err(|_| OutputError::OwnerStopped)?;
        response.recv().map_err(|_| OutputError::OwnerStopped)?
    }

    fn sender(&self, id: &OutputId) -> Result<mpsc::Sender<OwnerCommand>, OutputError> {
        self.owners
            .lock()
            .map_err(|_| OutputError::OwnerStopped)?
            .get(id)
            .map(|owner| owner.sender.clone())
            .ok_or(OutputError::UnknownOutput)
    }

    fn shared(&self, id: &OutputId) -> Result<Arc<Mutex<SharedOutput>>, OutputError> {
        self.owners
            .lock()
            .map_err(|_| OutputError::OwnerStopped)?
            .get(id)
            .map(|owner| Arc::clone(&owner.shared))
            .ok_or(OutputError::UnknownOutput)
    }
}

impl Default for OutputManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for OutputManager {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
