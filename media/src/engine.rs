use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};

use thiserror::Error;

use crate::engine_owner::owner_thread;
use crate::protocol::{Command, CommandId, CommandKind, EngineEvent, OutputId, TrackId};
use crate::{
    source_capture::{SourceCapture, SourceCaptureRegistry},
    tap::TapChannels,
};

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub(super) pipeline_description: String,
    pub(super) outputs: Vec<(OutputId, String)>,
    pub(super) tracks: Vec<(TrackId, String)>,
    pub(super) audio_routes: Vec<(TrackId, String)>,
}

impl EngineConfig {
    pub fn new<'a>(
        pipeline_description: &str,
        outputs: impl IntoIterator<Item = (OutputId, &'a str)>,
    ) -> Self {
        Self {
            pipeline_description: pipeline_description.to_owned(),
            outputs: outputs
                .into_iter()
                .map(|(id, valve)| (id, valve.to_owned()))
                .collect(),
            tracks: Vec::new(),
            audio_routes: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_track(mut self, track_id: TrackId, timing_element: &str) -> Self {
        self.tracks.push((track_id, timing_element.to_owned()));
        self
    }

    #[must_use]
    pub fn with_audio_route(mut self, track_id: TrackId, mixer_pad: &str) -> Self {
        self.audio_routes.push((track_id, mixer_pad.to_owned()));
        self
    }
}

#[derive(Debug)]
pub struct MediaEngine {
    sender: Option<Sender<EngineRequest>>,
    owner: Option<JoinHandle<()>>,
    source_captures: std::sync::Arc<SourceCaptureRegistry>,
}

impl MediaEngine {
    pub fn spawn(config: EngineConfig) -> Result<Self, EngineError> {
        let (sender, receiver) = mpsc::channel();
        let (startup_sender, startup_receiver) = mpsc::sync_channel(1);
        let source_captures = std::sync::Arc::new(SourceCaptureRegistry::default());
        let owner_source_captures = std::sync::Arc::clone(&source_captures);
        let owner = thread::Builder::new()
            .name("media-engine".to_owned())
            .spawn(move || {
                owner_thread(&config, &receiver, &startup_sender, owner_source_captures);
            })
            .map_err(EngineError::Spawn)?;
        let startup = startup_receiver
            .recv()
            .map_err(|_| EngineError::OwnerStopped)
            .and_then(std::convert::identity);
        if let Err(error) = startup {
            drop(sender);
            let _ = owner.join();
            return Err(error);
        }
        Ok(Self {
            sender: Some(sender),
            owner: Some(owner),
            source_captures,
        })
    }

    #[must_use]
    pub fn source_capture(&self, channels: &TapChannels) -> SourceCapture {
        self.source_captures.capture(channels)
    }

    pub fn execute(&self, command: Command) -> Result<EngineEvent, EngineError> {
        let (response_sender, response_receiver) = mpsc::sync_channel(1);
        self.sender
            .as_ref()
            .ok_or(EngineError::OwnerStopped)?
            .send(EngineRequest {
                command,
                response_sender,
            })
            .map_err(|_| EngineError::OwnerStopped)?;
        response_receiver
            .recv()
            .map_err(|_| EngineError::OwnerStopped)
    }
}

impl Drop for MediaEngine {
    fn drop(&mut self) {
        if let Some(sender) = self.sender.as_ref()
            && let Ok(id) = CommandId::new("engine-drop-shutdown")
        {
            let (response_sender, response_receiver) = mpsc::sync_channel(1);
            let _ = sender.send(EngineRequest {
                command: Command {
                    id,
                    expected_engine_run_id: None,
                    kind: CommandKind::Shutdown,
                },
                response_sender,
            });
            let _ = response_receiver.recv_timeout(std::time::Duration::from_secs(2));
        }
        let sender = self.sender.take();
        drop(sender);
        if let Some(owner) = self.owner.take() {
            let _ = owner.join();
        }
    }
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("GStreamer engine setup failed: {0}")]
    Gstreamer(String),
    #[error("media owner thread stopped")]
    OwnerStopped,
    #[error("failed to spawn media owner thread: {0}")]
    Spawn(std::io::Error),
}

#[derive(Debug)]
pub(super) struct EngineRequest {
    pub(super) command: Command,
    pub(super) response_sender: mpsc::SyncSender<EngineEvent>,
}
