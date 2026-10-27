use serde::{Deserialize, Serialize};

use crate::protocol::OutputId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputSnapshot {
    pub id: OutputId,
    pub desired_running: bool,
    pub observed: OutputObservedState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputObservedState {
    Failed,
    Finalizing,
    Reconnecting,
    Running,
    Starting,
    Stopped,
}
