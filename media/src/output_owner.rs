use std::{
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::protocol::EngineClockMapping;
use crate::{
    output::{OutputDefinition, OutputError, OutputSnapshot, OutputState, RecordingSegment},
    output_catalog,
    output_runtime::{ActivePipeline, PipelinePoll},
};

const OWNER_TICK: Duration = Duration::from_millis(100);
const RETRY_DELAY: Duration = Duration::from_secs(1);

pub struct SharedOutput {
    pub(crate) snapshot: OutputSnapshot,
    pub(crate) recordings: Vec<RecordingSegment>,
}

pub enum OwnerCommand {
    Start {
        clock: Option<EngineClockMapping>,
        reply: mpsc::SyncSender<Result<(), OutputError>>,
    },
    Stop(mpsc::SyncSender<Result<(), OutputError>>),
    Shutdown(mpsc::SyncSender<Result<(), OutputError>>),
}

pub struct SpawnedOwner {
    pub sender: mpsc::Sender<OwnerCommand>,
    pub shared: Arc<Mutex<SharedOutput>>,
    pub worker: JoinHandle<()>,
}

impl OwnerCommand {
    pub const fn stop(reply: mpsc::SyncSender<Result<(), OutputError>>) -> Self {
        Self::Stop(reply)
    }

    pub const fn shutdown(reply: mpsc::SyncSender<Result<(), OutputError>>) -> Self {
        Self::Shutdown(reply)
    }
}

pub fn spawn_owner(definition: OutputDefinition) -> Result<SpawnedOwner, OutputError> {
    let (sender, receiver) = mpsc::channel();
    let recordings = match &definition {
        OutputDefinition::Recording(recording) => output_catalog::load(&recording.directory)?,
        OutputDefinition::Rtmps { .. } => Vec::new(),
    };
    let shared = Arc::new(Mutex::new(SharedOutput {
        snapshot: OutputSnapshot {
            desired: OutputState::Stopped,
            actual: OutputState::Stopped,
            last_media_at_ns: None,
            error: None,
        },
        recordings,
    }));
    let owner_shared = Arc::clone(&shared);
    let worker = thread::Builder::new()
        .name(format!("output-{}", definition.id().as_str()))
        .spawn(move || owner_loop(&definition, &receiver, &owner_shared))
        .map_err(|_| OutputError::OwnerStopped)?;
    Ok(SpawnedOwner {
        sender,
        shared,
        worker,
    })
}

fn owner_loop(
    definition: &OutputDefinition,
    receiver: &mpsc::Receiver<OwnerCommand>,
    shared: &Arc<Mutex<SharedOutput>>,
) {
    let mut active: Option<ActivePipeline> = None;
    let mut retry_at: Option<Instant> = None;
    loop {
        match receiver.recv_timeout(OWNER_TICK) {
            Ok(OwnerCommand::Start { clock, reply }) => {
                if active.is_some() || retry_at.is_some() {
                    let _ = reply.send(Ok(()));
                    continue;
                }
                set_starting(shared);
                match ActivePipeline::start(definition, clock.as_ref()) {
                    Ok(pipeline) => {
                        active = Some(pipeline);
                        let _ = reply.send(Ok(()));
                    }
                    Err(error) => {
                        schedule_failure(
                            definition,
                            shared,
                            &mut retry_at,
                            Some(error.to_string()),
                        );
                        let result = match definition {
                            OutputDefinition::Rtmps { .. } => Ok(()),
                            OutputDefinition::Recording(_) => Err(error),
                        };
                        let _ = reply.send(result);
                    }
                }
            }
            Ok(OwnerCommand::Stop(reply)) => {
                retry_at = None;
                let result = stop_active(definition, &mut active, shared);
                set_stopped(shared, result.as_ref().err());
                let _ = reply.send(result);
            }
            Ok(OwnerCommand::Shutdown(reply)) => {
                let result = stop_active(definition, &mut active, shared);
                set_stopped(shared, result.as_ref().err());
                let _ = reply.send(result);
                return;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = stop_active(definition, &mut active, shared);
                return;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if let Some(pipeline) = active.as_mut() {
            match pipeline.poll(shared) {
                PipelinePoll::Progress => set_running(shared),
                PipelinePoll::NoProgress => {}
                PipelinePoll::Unhealthy(error) => {
                    if let Some(failed) = active.take() {
                        failed.fail(shared);
                    }
                    schedule_failure(definition, shared, &mut retry_at, Some(error));
                }
            }
        }
        if retry_at.is_some_and(|deadline| Instant::now() >= deadline) {
            set_starting(shared);
            match ActivePipeline::start(definition, None) {
                Ok(pipeline) => {
                    active = Some(pipeline);
                    retry_at = None;
                }
                Err(error) => {
                    schedule_failure(definition, shared, &mut retry_at, Some(error.to_string()));
                }
            }
        }
    }
}

fn stop_active(
    definition: &OutputDefinition,
    active: &mut Option<ActivePipeline>,
    shared: &Arc<Mutex<SharedOutput>>,
) -> Result<(), OutputError> {
    update(shared, |state| {
        state.snapshot.desired = OutputState::Stopped;
        state.snapshot.actual = OutputState::Finalizing;
    });
    active
        .take()
        .map_or(Ok(()), |pipeline| pipeline.stop(definition, shared))
}

fn schedule_failure(
    definition: &OutputDefinition,
    shared: &Arc<Mutex<SharedOutput>>,
    retry_at: &mut Option<Instant>,
    detail: Option<String>,
) {
    match definition {
        OutputDefinition::Rtmps { .. } => {
            *retry_at = Some(Instant::now() + RETRY_DELAY);
            update(shared, |state| {
                state.snapshot.actual = OutputState::Retrying;
                state.snapshot.error = detail
                    .or_else(|| Some("Network destination is unavailable; retrying.".to_owned()));
            });
        }
        OutputDefinition::Recording(_) => {
            *retry_at = None;
            update(shared, |state| {
                state.snapshot.actual = OutputState::Failed;
                state.snapshot.error =
                    detail.or_else(|| Some("Recording pipeline failed.".to_owned()));
            });
        }
    }
}

fn set_starting(shared: &Arc<Mutex<SharedOutput>>) {
    update(shared, |state| {
        state.snapshot.desired = OutputState::Running;
        state.snapshot.actual = OutputState::Starting;
        state.snapshot.error = None;
    });
}

fn set_running(shared: &Arc<Mutex<SharedOutput>>) {
    update(shared, |state| {
        state.snapshot.actual = OutputState::Running;
        state.snapshot.error = None;
    });
}

fn set_stopped(shared: &Arc<Mutex<SharedOutput>>, error: Option<&OutputError>) {
    update(shared, |state| {
        state.snapshot.actual = if error.is_some() {
            OutputState::Failed
        } else {
            OutputState::Stopped
        };
        state.snapshot.error = error.map(std::string::ToString::to_string);
    });
}

fn update(shared: &Arc<Mutex<SharedOutput>>, apply: impl FnOnce(&mut SharedOutput)) {
    if let Ok(mut state) = shared.lock() {
        apply(&mut state);
    }
}
