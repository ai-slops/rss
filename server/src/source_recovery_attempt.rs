use crate::{
    source_recovery::{MAXIMUM_ATTEMPTS, SourceContext},
    source_recovery_state::{cancel, ensure_eligible, parse},
    source_recovery_types::{ReceiverState, RecoveryAction, RecoveryRecord, RetryResult},
};

pub fn maybe_action(
    record: &mut RecoveryRecord,
    source: &SourceContext,
    server_run_id: &str,
    now: u64,
) -> Option<RecoveryAction> {
    if ensure_eligible(record, source, now).is_err() {
        if record.retry.in_flight.is_some() {
            cancel(record);
        }
        return None;
    }
    if let Some(action) = &record.retry.in_flight {
        if action.server_run_id == server_run_id {
            return Some(action.clone());
        }
        cancel(record);
    }
    let eligible_at = record
        .retry
        .next_eligible_at_ns
        .as_deref()
        .and_then(parse)
        .unwrap_or(0);
    if record.receiver.state != ReceiverState::Stalled || now < eligible_at {
        return None;
    }
    if record.retry.attempts >= MAXIMUM_ATTEMPTS {
        record.retry.last_result = RetryResult::Exhausted;
        return None;
    }
    let controller = record.controller.as_ref()?;
    record.retry.attempts += 1;
    let backoff = 1_u64 << u32::from(record.retry.attempts - 1);
    record.retry.next_eligible_at_ns =
        Some(now.saturating_add(backoff * 1_000_000_000).to_string());
    record.retry.last_result = RetryResult::Pending;
    record.retry.healthy_observations = 0;
    record.retry.action_grace_started_ns = None;
    record.retry.baseline_generation = record.receiver.continuity_generation.clone();
    let action = RecoveryAction {
        kind: "restartPublisher".into(),
        command_id: uuid::Uuid::new_v4().to_string(),
        server_run_id: server_run_id.to_owned(),
        controller_id: controller.controller_id.clone(),
        process_epoch: controller.process_epoch.clone(),
        intent_epoch: controller.intent_epoch.clone(),
        attempt: record.retry.attempts,
    };
    record.retry.in_flight = Some(action.clone());
    Some(action)
}
