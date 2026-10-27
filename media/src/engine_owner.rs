use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver};
use std::time::{SystemTime, UNIX_EPOCH};

use gst::prelude::*;
use gstreamer as gst;
use uuid::Uuid;

use crate::engine::{EngineConfig, EngineError, EngineRequest};
use crate::engine_composition::CompositionLayerRuntime;
use crate::engine_sources::SourceRuntime;
use crate::protocol::{
    Command, CommandId, CommandKind, EngineEvent, EngineRunId, EventKind, Nanoseconds, OutputId,
    OutputObservedState, ProgramRevision, ScenePlacement, Sequence, TrackId,
};

#[derive(Debug, Clone)]
pub struct OutputControl {
    pub valve_name: String,
    pub desired_running: bool,
    pub observed: OutputObservedState,
}

pub struct OwnerState {
    pub pipeline: gst::Pipeline,
    pub engine_run_id: EngineRunId,
    pub sequence: u64,
    pub outputs: HashMap<OutputId, OutputControl>,
    pub completed: HashMap<CommandId, EngineEvent>,
    pub prepared: Option<(ProgramRevision, Vec<ScenePlacement>)>,
    pub prepared_composition: Option<(ProgramRevision, Vec<crate::protocol::CompositionPlacement>)>,
    pub program_revision: Option<ProgramRevision>,
    pub preview_layers: Vec<CompositionLayerRuntime>,
    pub program_layers: Vec<CompositionLayerRuntime>,
    pub tracks: HashMap<TrackId, String>,
    pub track_history: HashMap<TrackId, String>,
    pub timing_controls:
        HashMap<TrackId, std::sync::Arc<crate::engine_source_progress::TimingControl>>,
    pub video_tees: HashMap<TrackId, String>,
    pub audio_routes: HashMap<TrackId, String>,
    pub sources: HashMap<crate::protocol::SourceId, SourceRuntime>,
}

pub fn owner_thread(
    config: &EngineConfig,
    receiver: &Receiver<EngineRequest>,
    startup_sender: &mpsc::SyncSender<Result<(), EngineError>>,
) {
    let setup = setup_owner(config);
    let Ok(mut owner) = setup else {
        let error = setup.err().unwrap_or(EngineError::OwnerStopped);
        let _ = startup_sender.send(Err(error));
        return;
    };
    if startup_sender.send(Ok(())).is_err() {
        stop_pipeline(&owner.pipeline);
        return;
    }
    while let Ok(request) = receiver.recv() {
        let shutdown = matches!(request.command.kind, CommandKind::Shutdown);
        let event = owner.handle(request.command);
        if request.response_sender.send(event).is_err() || shutdown {
            break;
        }
    }
    stop_pipeline(&owner.pipeline);
}

fn setup_owner(config: &EngineConfig) -> Result<OwnerState, EngineError> {
    gst::init().map_err(|error| EngineError::Gstreamer(error.to_string()))?;
    let engine_run_id = EngineRunId::new(&Uuid::now_v7().to_string())
        .map_err(|error| EngineError::Gstreamer(error.to_string()))?;
    let description = config
        .pipeline_description
        .replace("{engine_run_id}", engine_run_id.as_str());
    let element = gst::parse::launch(&description)
        .map_err(|error| EngineError::Gstreamer(error.to_string()))?;
    let pipeline = element
        .downcast::<gst::Pipeline>()
        .map_err(|_| EngineError::Gstreamer("description did not produce a Pipeline".to_owned()))?;
    pipeline
        .set_state(gst::State::Playing)
        .map_err(|error| EngineError::Gstreamer(format!("start pipeline: {error:?}")))?;
    let outputs = config
        .outputs
        .iter()
        .map(|(id, valve_name)| {
            (
                id.clone(),
                OutputControl {
                    valve_name: valve_name.clone(),
                    desired_running: false,
                    observed: OutputObservedState::Stopped,
                },
            )
        })
        .collect();
    Ok(OwnerState {
        pipeline,
        engine_run_id,
        sequence: 0,
        outputs,
        completed: HashMap::new(),
        prepared: None,
        prepared_composition: None,
        program_revision: None,
        preview_layers: Vec::new(),
        program_layers: Vec::new(),
        tracks: config.tracks.iter().cloned().collect(),
        track_history: HashMap::new(),
        timing_controls: HashMap::new(),
        video_tees: HashMap::new(),
        audio_routes: config.audio_routes.iter().cloned().collect(),
        sources: HashMap::new(),
    })
}

impl OwnerState {
    fn handle(&mut self, command: Command) -> EngineEvent {
        if let Some(event) = self.completed.get(&command.id) {
            return event.clone();
        }
        self.sequence = self.sequence.saturating_add(1);
        let kind = if command
            .expected_engine_run_id
            .as_ref()
            .is_some_and(|expected| expected != &self.engine_run_id)
        {
            EventKind::Rejected {
                reason: "engine_run_id mismatch".to_owned(),
            }
        } else {
            self.apply(&command.kind)
        };
        let event = EngineEvent {
            sequence: Sequence::new(self.sequence),
            engine_run_id: self.engine_run_id.clone(),
            command_id: Some(command.id.clone()),
            at: now_nanoseconds(),
            kind,
        };
        self.completed.insert(command.id, event.clone());
        event
    }
}

fn stop_pipeline(pipeline: &gst::Pipeline) {
    let _ = pipeline.send_event(gst::event::Eos::new());
    if let Some(bus) = pipeline.bus() {
        let _ = bus.timed_pop_filtered(
            gst::ClockTime::from_seconds(10),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        );
    }
    let _ = pipeline.set_state(gst::State::Null);
}

pub fn now_nanoseconds() -> Nanoseconds {
    let value = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            duration.as_nanos().try_into().unwrap_or(u64::MAX)
        });
    Nanoseconds::new(value)
}
