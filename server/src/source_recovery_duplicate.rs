use sha2::{Digest, Sha256};

use crate::{
    error::{AppError, Result},
    source_recovery_types::*,
};

pub enum PollPreflight {
    Fresh,
    Duplicate(Box<PollResponse>),
}

pub fn preflight(
    record: &RecoveryRecord,
    request: &PollRequest,
    source_enabled: bool,
    server_run_id: &str,
    now: u64,
) -> Result<PollPreflight> {
    let controller = record.controller.as_ref().ok_or(AppError::Unauthorized)?;
    if controller.controller_id != request.controller_id {
        return Err(AppError::Unauthorized);
    }
    let process = decimal(&request.process_epoch, "processEpoch")?;
    let previous_process = decimal(&controller.process_epoch, "processEpoch")?;
    let poll = decimal(&request.poll_seq, "pollSeq")?;
    let previous_poll = decimal(&controller.poll_seq, "pollSeq")?;
    if process < previous_process || process == previous_process && poll < previous_poll {
        return Err(AppError::Conflict(
            "Recovery controller epoch is stale.".into(),
        ));
    }
    if process == previous_process && poll == previous_poll {
        if controller.last_poll_fingerprint.as_deref() != Some(&fingerprint(request)?) {
            return Err(AppError::Conflict(
                "Duplicate poll sequence payload differs.".into(),
            ));
        }
        let mut response = controller
            .cached_response
            .as_deref()
            .cloned()
            .ok_or_else(|| AppError::Conflict("Duplicate poll response is unavailable.".into()))?;
        if !record.policy.enabled
            || record.policy.maintenance
            || !source_enabled
            || !controller.armed
            || controller.local_desired != LocalDesired::Running
            || controller
                .last_poll_ns
                .as_deref()
                .and_then(|value| value.parse::<u64>().ok())
                .is_none_or(|last| now.saturating_sub(last) > crate::source_recovery::LEASE_NS)
            || response.server_run_id != server_run_id
            || response.status.policy_version != record.policy_version
            || response.action.as_ref().is_some_and(|cached| {
                !record.retry.in_flight.as_ref().is_some_and(|current| {
                    current.command_id == cached.command_id
                        && current.process_epoch == controller.process_epoch
                        && current.intent_epoch == controller.intent_epoch
                })
            })
        {
            response.action = None;
            response
                .status
                .policy_version
                .clone_from(&record.policy_version);
            response.status.policy = record.policy.clone();
            response.status.source_enabled = source_enabled;
            response.server_run_id = server_run_id.to_owned();
        }
        return Ok(PollPreflight::Duplicate(Box::new(response)));
    }
    Ok(PollPreflight::Fresh)
}

pub fn cache(record: &mut RecoveryRecord, response: &PollResponse) {
    if let Some(controller) = record.controller.as_mut() {
        controller.cached_response = Some(Box::new(response.clone()));
    }
}
pub fn fingerprint(request: &PollRequest) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(request).map_err(|error| AppError::Internal(error.into()))?
        )
    ))
}
fn decimal(value: &str, field: &str) -> Result<u64> {
    value.parse().map_err(|_| {
        AppError::Invalid(format!(
            "{field} must be a decimal-string nonnegative integer."
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_recovery::{SourceContext, SourceRecoveryManager};
    use crate::source_recovery_state::status;
    use crate::store::Store;

    #[test]
    fn a_cancelled_cached_action_cannot_be_reauthorized_after_policy_reenable() -> anyhow::Result<()>
    {
        let directory = tempfile::tempdir()?;
        let store = Store::open(&directory.path().join("project.sqlite"))?;
        let mut manager = SourceRecoveryManager::load(&store)?;
        let source = SourceContext {
            id: "source".into(),
            enabled: true,
            srt: true,
        };
        let (enrolled, _) = manager.enroll(&store, source, false, 0)?;
        let mut record: RecoveryRecord = serde_json::from_value(
            store
                .plugin_get(crate::source_recovery::NAMESPACE, "source")?
                .expect("record"),
        )?;
        record.policy.enabled = true;
        record.policy_version = "2".into();
        let request = PollRequest {
            schema_version: 1,
            controller_id: enrolled.controller_id.clone(),
            process_epoch: "1".into(),
            poll_seq: "4".into(),
            intent_epoch: "2".into(),
            local_desired: LocalDesired::Running,
            publisher_state: PublisherState::Sending,
            last_action: None,
        };
        let action = RecoveryAction {
            kind: "restartPublisher".into(),
            command_id: uuid::Uuid::new_v4().to_string(),
            server_run_id: "run".into(),
            controller_id: enrolled.controller_id,
            process_epoch: "1".into(),
            intent_epoch: "2".into(),
            attempt: 1,
        };
        let source = SourceContext {
            id: "source".into(),
            enabled: true,
            srt: true,
        };
        let cached = PollResponse {
            status: status(&record, &source, 100),
            server_run_id: "run".into(),
            process_epoch: "1".into(),
            poll_seq: "4".into(),
            intent_epoch: "2".into(),
            next_poll_ms: 1000,
            action: Some(action),
        };
        let controller = record.controller.as_mut().expect("controller");
        controller.process_epoch = "1".into();
        controller.poll_seq = "4".into();
        controller.intent_epoch = "2".into();
        controller.local_desired = LocalDesired::Running;
        controller.armed = true;
        controller.last_poll_ns = Some("100".into());
        controller.last_poll_fingerprint = Some(fingerprint(&request)?);
        controller.cached_response = Some(Box::new(cached));
        record.policy_version = "4".into();
        record.retry.in_flight = None;
        let PollPreflight::Duplicate(response) = preflight(&record, &request, true, "run", 101)?
        else {
            panic!("expected duplicate")
        };
        assert!(response.action.is_none());
        Ok(())
    }
}
