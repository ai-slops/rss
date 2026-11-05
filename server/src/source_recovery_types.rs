use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryPolicy {
    pub enabled: bool,
    pub maintenance: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerRecord {
    pub controller_id: String,
    pub token_hash: String,
    pub last_poll_ns: Option<String>,
    pub process_epoch: String,
    pub poll_seq: String,
    pub intent_epoch: String,
    pub local_desired: LocalDesired,
    pub armed: bool,
    #[serde(default)]
    pub last_poll_fingerprint: Option<String>,
    #[serde(default = "stopped_publisher")]
    pub publisher_state: PublisherState,
    #[serde(default)]
    pub cached_response: Option<Box<PollResponse>>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryRecord {
    pub attempts: u8,
    pub next_eligible_at_ns: Option<String>,
    pub last_result: RetryResult,
    pub in_flight: Option<RecoveryAction>,
    pub healthy_observations: u8,
    pub request_ids: Vec<String>,
    #[serde(default)]
    pub action_grace_started_ns: Option<String>,
    #[serde(default)]
    pub baseline_generation: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiverRecord {
    pub continuity_generation: Option<String>,
    pub video_frames: Option<String>,
    pub audio_buffers: Option<String>,
    pub last_video_pts_ns: Option<String>,
    pub last_progress_ns: Option<String>,
    pub state: ReceiverState,
    #[serde(default)]
    pub generation_started_ns: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryRecord {
    pub schema_version: u8,
    pub source_id: String,
    pub policy_version: String,
    pub policy: RecoveryPolicy,
    pub controller: Option<ControllerRecord>,
    pub retry: RetryRecord,
    pub receiver: ReceiverRecord,
    #[serde(default)]
    pub server_run_id: Option<String>,
}

impl RecoveryRecord {
    pub fn new(source_id: &str) -> Self {
        Self {
            schema_version: 1,
            source_id: source_id.to_owned(),
            policy_version: "1".into(),
            policy: RecoveryPolicy::default(),
            controller: None,
            retry: RetryRecord::default(),
            receiver: ReceiverRecord::default(),
            server_run_id: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReceiverState {
    #[default]
    Unknown,
    Warming,
    Healthy,
    Stalled,
    SourceDisabled,
    ControllerOffline,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RetryResult {
    #[default]
    Never,
    Pending,
    Started,
    Failed,
    Recovered,
    Exhausted,
    Cancelled,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LocalDesired {
    Running,
    Stopped,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PublisherState {
    Created,
    Ready,
    Starting,
    Sending,
    Reconnecting,
    Stopping,
    Stopped,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryAction {
    pub kind: String,
    pub command_id: String,
    pub server_run_id: String,
    pub controller_id: String,
    pub process_epoch: String,
    pub intent_epoch: String,
    pub attempt: u8,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PollRequest {
    pub schema_version: u8,
    pub controller_id: String,
    pub process_epoch: String,
    pub poll_seq: String,
    pub intent_epoch: String,
    pub local_desired: LocalDesired,
    pub publisher_state: PublisherState,
    pub last_action: Option<ActionResult>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionResult {
    pub command_id: String,
    pub result: ActionOutcome,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionOutcome {
    Started,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryStatus {
    pub schema_version: u8,
    pub source_id: String,
    pub policy_version: String,
    pub source_enabled: bool,
    pub policy: RecoveryPolicy,
    pub controller: ControllerStatus,
    pub receiver: ReceiverStatus,
    pub retry: RetryStatus,
    pub capabilities: Capabilities,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerStatus {
    pub enrolled: bool,
    pub controller_id: Option<String>,
    pub online: bool,
    pub process_epoch: Option<String>,
    pub local_desired: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiverStatus {
    pub state: ReceiverState,
    pub continuity_generation: Option<String>,
    pub video_frames: Option<String>,
    pub audio_buffers: Option<String>,
    pub last_video_pts_ns: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryStatus {
    pub attempts: u8,
    pub maximum_attempts: u8,
    pub next_eligible_at_ns: Option<String>,
    pub last_result: RetryResult,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub publisher_restart: bool,
    pub process_restart: bool,
    pub freeze_detection: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PollResponse {
    #[serde(flatten)]
    pub status: RecoveryStatus,
    pub server_run_id: String,
    pub process_epoch: String,
    pub poll_seq: String,
    pub intent_epoch: String,
    pub next_poll_ms: u16,
    pub action: Option<RecoveryAction>,
}

const fn stopped_publisher() -> PublisherState {
    PublisherState::Stopped
}
