use serde_json::Value;

use crate::{
    coordinator::Owner,
    error::{AppError, Result},
    model::SourceAdapter,
    source_recovery::{HealthObservation, SourceContext},
    source_recovery_duplicate::PollPreflight,
    source_recovery_types::{PollRequest, RecoveryPolicy},
};

pub enum RecoveryCommand {
    Status {
        source_id: String,
    },
    Policy {
        source_id: String,
        expected: String,
        policy: RecoveryPolicy,
    },
    Enroll {
        source_id: String,
        rotate: bool,
    },
    Revoke {
        source_id: String,
    },
    Retry {
        source_id: String,
        request_id: String,
    },
    Poll {
        source_id: String,
        token: String,
        request: PollRequest,
    },
}

impl Owner {
    pub(crate) fn recovery(&mut self, command: RecoveryCommand) -> Result<Value> {
        let now = crate::coordinator::now_ns()
            .parse::<u64>()
            .map_err(|error| AppError::Internal(error.into()))?;
        let value = match command {
            RecoveryCommand::Poll {
                source_id,
                token,
                request,
            } => {
                self.recovery.authenticate(&source_id, &token)?;
                let source = self.source_context(&source_id)?;
                match self.recovery.preflight_poll(
                    &source,
                    &request,
                    self.media.engine_run_id(),
                    now,
                )? {
                    PollPreflight::Duplicate(response) => serde_json::to_value(*response),
                    PollPreflight::Fresh => {
                        self.observe_recovery(&source_id, now)?;
                        let source = self.source_context(&source_id)?;
                        serde_json::to_value(self.recovery.poll(
                            &self.store,
                            source,
                            request,
                            self.media.engine_run_id(),
                            now,
                        )?)
                    }
                }
            }
            RecoveryCommand::Status { source_id } => {
                self.observe_recovery(&source_id, now)?;
                let source = self.source_context(&source_id)?;
                serde_json::to_value(self.recovery.status(source, now))
            }
            RecoveryCommand::Policy {
                source_id,
                expected,
                policy,
            } => {
                self.observe_recovery(&source_id, now)?;
                let source = self.source_context(&source_id)?;
                serde_json::to_value(self.recovery.set_policy(
                    &self.store,
                    source,
                    &expected,
                    policy,
                    now,
                )?)
            }
            RecoveryCommand::Enroll { source_id, rotate } => {
                let source = self.source_context(&source_id)?;
                let (enrollment, status) =
                    self.recovery.enroll(&self.store, source, rotate, now)?;
                serde_json::to_value(
                    serde_json::json!({"schemaVersion":1,"sourceId":source_id,"controllerId":enrollment.controller_id,"controllerToken":enrollment.controller_token,"controlPath":format!("/api/v1/source-recovery/{source_id}/poll"),"policyVersion":status.policy_version}),
                )
            }
            RecoveryCommand::Revoke { source_id } => {
                let source = self.source_context(&source_id)?;
                serde_json::to_value(self.recovery.revoke(&self.store, source, now)?)
            }
            RecoveryCommand::Retry {
                source_id,
                request_id,
            } => {
                self.observe_recovery(&source_id, now)?;
                let source = self.source_context(&source_id)?;
                serde_json::to_value(self.recovery.manual_retry(
                    &self.store,
                    source,
                    &request_id,
                    now,
                )?)
            }
        };
        value.map_err(|error| AppError::Internal(error.into()))
    }

    fn source_context(&self, id: &str) -> Result<SourceContext> {
        let source = self
            .project
            .sources
            .iter()
            .find(|source| source.id.0 == id)
            .ok_or_else(|| AppError::Invalid("Source does not exist.".into()))?;
        if !matches!(source.adapter, SourceAdapter::Srt { .. }) {
            return Err(AppError::Conflict(
                "Publisher recovery requires an SRT source.".into(),
            ));
        }
        Ok(SourceContext {
            id: source.id.0.clone(),
            enabled: source.enabled,
            srt: matches!(source.adapter, SourceAdapter::Srt { .. }),
        })
    }

    fn observe_recovery(&mut self, id: &str, now: u64) -> Result<()> {
        let source = self.source_context(id)?;
        let (sources, _, _) = self.media.runtime_snapshot()?;
        let native = sources.iter().find(|value| value.id.as_str() == id);
        let Some(native) = native else {
            return Ok(());
        };
        self.recovery.observe(
            &self.store,
            source,
            HealthObservation {
                generation: native.continuity_generation.value().to_string(),
                video_frames: native.video_received_frames.value().to_string(),
                audio_buffers: native.audio_received_buffers.value().to_string(),
                last_video_pts_ns: native
                    .video_received_pts
                    .map(|value| value.value().to_string()),
            },
            now,
        )
    }
}
