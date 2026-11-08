use crate::{
    error::{AppError, Result},
    source_recovery::{LEASE_NS, MAXIMUM_ATTEMPTS, NAMESPACE, SourceContext},
    source_recovery_duplicate::fingerprint,
    source_recovery_types::*,
    source_recovery_validation::parse_required,
    store::Store,
};

pub fn status(record: &RecoveryRecord, source: &SourceContext, now: u64) -> RecoveryStatus {
    let online = record
        .controller
        .as_ref()
        .and_then(|controller| controller.last_poll_ns.as_deref())
        .and_then(parse)
        .is_some_and(|last| now.saturating_sub(last) <= LEASE_NS);
    RecoveryStatus {
        schema_version: 1,
        source_id: source.id.to_owned(),
        policy_version: record.policy_version.clone(),
        source_enabled: source.enabled,
        policy: record.policy.clone(),
        controller: ControllerStatus {
            enrolled: record.controller.is_some(),
            controller_id: record
                .controller
                .as_ref()
                .map(|value| value.controller_id.clone()),
            online,
            process_epoch: record.controller.as_ref().and_then(|value| {
                (value.process_epoch != "0").then(|| value.process_epoch.clone())
            }),
            local_desired: record
                .controller
                .as_ref()
                .map_or("stopped", |value| match value.local_desired {
                    LocalDesired::Running => "running",
                    LocalDesired::Stopped => "stopped",
                })
                .to_owned(),
        },
        receiver: ReceiverStatus {
            state: if !source.enabled {
                ReceiverState::SourceDisabled
            } else if record.controller.is_some() && !online {
                ReceiverState::ControllerOffline
            } else {
                record.receiver.state
            },
            continuity_generation: record.receiver.continuity_generation.clone(),
            video_frames: record.receiver.video_frames.clone(),
            audio_buffers: record.receiver.audio_buffers.clone(),
            last_video_pts_ns: record.receiver.last_video_pts_ns.clone(),
        },
        retry: RetryStatus {
            attempts: record.retry.attempts,
            maximum_attempts: MAXIMUM_ATTEMPTS,
            next_eligible_at_ns: record.retry.next_eligible_at_ns.clone(),
            last_result: record.retry.last_result,
        },
        capabilities: Capabilities {
            publisher_restart: true,
            process_restart: false,
            freeze_detection: "decoded_counter_and_pts_progress".into(),
        },
    }
}

pub fn persist(store: &Store, record: &RecoveryRecord) -> Result<()> {
    store
        .plugin_put(
            NAMESPACE,
            &record.source_id,
            &serde_json::to_value(record).map_err(|error| AppError::Internal(error.into()))?,
        )
        .map_err(AppError::Internal)
}

pub fn cancel(record: &mut RecoveryRecord) {
    record.retry.in_flight = None;
    record.retry.last_result = RetryResult::Cancelled;
    record.retry.healthy_observations = 0;
}

pub fn ensure_eligible(record: &RecoveryRecord, source: &SourceContext, now: u64) -> Result<()> {
    let controller = record
        .controller
        .as_ref()
        .ok_or_else(|| AppError::Conflict("Recovery controller is not enrolled.".into()))?;
    let online = controller
        .last_poll_ns
        .as_deref()
        .and_then(parse)
        .is_some_and(|last| now.saturating_sub(last) <= LEASE_NS);
    if !source.srt
        || !record.policy.enabled
        || record.policy.maintenance
        || !source.enabled
        || !online
        || !controller.armed
        || controller.local_desired != LocalDesired::Running
    {
        return Err(AppError::Conflict(
            "Recovery retry is not currently eligible.".into(),
        ));
    }
    Ok(())
}

pub fn apply_poll(record: &mut RecoveryRecord, request: &PollRequest, now: u64) -> Result<()> {
    let fingerprint = fingerprint(request)?;
    let mut should_cancel = false;
    let mut fresh_start = false;
    {
        let controller = record.controller.as_mut().ok_or(AppError::Unauthorized)?;
        if controller.controller_id != request.controller_id {
            return Err(AppError::Unauthorized);
        }
        let process = parse_required(&request.process_epoch, "processEpoch")?;
        let previous_process = parse_required(&controller.process_epoch, "processEpoch")?;
        let poll = parse_required(&request.poll_seq, "pollSeq")?;
        let previous_poll = parse_required(&controller.poll_seq, "pollSeq")?;
        let intent = parse_required(&request.intent_epoch, "intentEpoch")?;
        let previous_intent = parse_required(&controller.intent_epoch, "intentEpoch")?;
        if process < previous_process
            || process == previous_process && poll < previous_poll
            || intent < previous_intent
        {
            return Err(AppError::Conflict(
                "Recovery controller epoch is stale.".into(),
            ));
        }
        if process == previous_process && poll == previous_poll {
            if controller.last_poll_fingerprint.as_deref() != Some(&fingerprint) {
                return Err(AppError::Conflict(
                    "Duplicate poll sequence payload differs.".into(),
                ));
            }
            controller.last_poll_ns = Some(now.to_string());
            return Ok(());
        }
        if controller
            .last_poll_ns
            .as_deref()
            .and_then(parse)
            .is_some_and(|last| now.saturating_sub(last) > LEASE_NS)
        {
            should_cancel = true;
        }
        if process > previous_process {
            controller.process_epoch = request.process_epoch.clone();
            controller.poll_seq = request.poll_seq.clone();
            controller.intent_epoch = request.intent_epoch.clone();
            controller.local_desired = LocalDesired::Stopped;
            controller.armed = false;
            should_cancel = true;
        } else {
            controller.poll_seq = request.poll_seq.clone();
            if intent > previous_intent {
                controller.intent_epoch = request.intent_epoch.clone();
                controller.local_desired = request.local_desired;
                controller.armed = request.local_desired == LocalDesired::Running;
                should_cancel = true;
                fresh_start = request.local_desired == LocalDesired::Running;
            }
        }
        controller.last_poll_ns = Some(now.to_string());
        controller.last_poll_fingerprint = Some(fingerprint);
        controller.publisher_state = request.publisher_state;
    }
    if should_cancel {
        cancel(record);
    }
    if fresh_start {
        record.receiver.state = ReceiverState::Warming;
        record.retry.action_grace_started_ns = Some(now.to_string());
        record.retry.next_eligible_at_ns = Some(now.saturating_add(3_000_000_000).to_string());
    }
    if let Some(result) = &request.last_action
        && record
            .retry
            .in_flight
            .as_ref()
            .is_some_and(|action| action.command_id == result.command_id)
    {
        record.retry.in_flight = None;
        record.retry.last_result = match result.result {
            ActionOutcome::Started => RetryResult::Started,
            ActionOutcome::Failed => RetryResult::Failed,
        };
        record.retry.healthy_observations = 0;
        if matches!(result.result, ActionOutcome::Started) {
            record.retry.action_grace_started_ns = Some(now.to_string());
            let deadline = now.saturating_add(3_000_000_000);
            let backoff = record
                .retry
                .next_eligible_at_ns
                .as_deref()
                .and_then(parse)
                .unwrap_or(0);
            record.retry.next_eligible_at_ns = Some(deadline.max(backoff).to_string());
            record.receiver.state = ReceiverState::Warming;
        }
    }
    Ok(())
}

pub fn parse(value: &str) -> Option<u64> {
    value.parse().ok()
}
