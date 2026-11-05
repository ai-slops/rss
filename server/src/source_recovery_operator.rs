use super::{
    Enrollment, MAXIMUM_ATTEMPTS, MAXIMUM_REQUEST_IDS, SourceContext, SourceRecoveryManager, hash,
};
use crate::{
    error::{AppError, Result},
    source_recovery_state::{cancel, ensure_eligible, persist, status},
    source_recovery_types::{
        ControllerRecord, LocalDesired, PublisherState, RecoveryPolicy, RecoveryStatus, RetryResult,
    },
    store::Store,
};

impl SourceRecoveryManager {
    pub fn set_policy(
        &mut self,
        store: &Store,
        source: SourceContext,
        expected: &str,
        policy: RecoveryPolicy,
        now: u64,
    ) -> Result<RecoveryStatus> {
        let mut record = self.record_for_source(&source.id);
        if expected != record.policy_version {
            return Err(AppError::Conflict(
                "Recovery policy version is stale.".into(),
            ));
        }
        let next = record
            .policy_version
            .parse::<u64>()
            .map_err(|error| AppError::Internal(error.into()))?
            .checked_add(1)
            .ok_or_else(|| AppError::Conflict("Recovery policy version is exhausted.".into()))?;
        record.policy = policy;
        record.policy_version = next.to_string();
        if !record.policy.enabled || record.policy.maintenance {
            cancel(&mut record);
        }
        persist(store, &record)?;
        let result = status(&record, &source, now);
        self.records.insert(source.id, record);
        Ok(result)
    }

    pub fn enroll(
        &mut self,
        store: &Store,
        source: SourceContext,
        rotate: bool,
        now: u64,
    ) -> Result<(Enrollment, RecoveryStatus)> {
        if !source.srt || !source.enabled {
            return Err(AppError::Conflict(
                "Recovery enrollment requires an enabled SRT source.".into(),
            ));
        }
        let mut record = self.record_for_source(&source.id);
        if record.controller.is_some() && !rotate {
            return Err(AppError::Conflict(
                "Recovery controller is already enrolled; rotate explicitly.".into(),
            ));
        }
        let controller_id = uuid::Uuid::new_v4().to_string();
        let controller_token = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        record.controller = Some(ControllerRecord {
            controller_id: controller_id.clone(),
            token_hash: hash(&controller_token),
            last_poll_ns: None,
            process_epoch: "0".into(),
            poll_seq: "0".into(),
            intent_epoch: "0".into(),
            local_desired: LocalDesired::Stopped,
            armed: false,
            last_poll_fingerprint: None,
            publisher_state: PublisherState::Stopped,
            cached_response: None,
        });
        cancel(&mut record);
        persist(store, &record)?;
        let result = status(&record, &source, now);
        self.records.insert(source.id, record);
        Ok((
            Enrollment {
                controller_id,
                controller_token,
            },
            result,
        ))
    }

    pub fn revoke(
        &mut self,
        store: &Store,
        source: SourceContext,
        now: u64,
    ) -> Result<RecoveryStatus> {
        let mut record = self.record_for_source(&source.id);
        record.controller = None;
        cancel(&mut record);
        persist(store, &record)?;
        let result = status(&record, &source, now);
        self.records.insert(source.id, record);
        Ok(result)
    }

    pub fn manual_retry(
        &mut self,
        store: &Store,
        source: SourceContext,
        request_id: &str,
        now: u64,
    ) -> Result<RecoveryStatus> {
        uuid::Uuid::parse_str(request_id)
            .map_err(|_| AppError::Invalid("requestId must be a UUID.".into()))?;
        let mut record = self.record_for_source(&source.id);
        if record.retry.request_ids.iter().any(|id| id == request_id) {
            return Ok(status(&record, &source, now));
        }
        ensure_eligible(&record, &source, now)?;
        record.retry.request_ids.push(request_id.to_owned());
        if record.retry.request_ids.len() > MAXIMUM_REQUEST_IDS {
            record.retry.request_ids.remove(0);
        }
        if record.retry.attempts >= MAXIMUM_ATTEMPTS {
            record.retry.attempts = 0;
        }
        record.retry.next_eligible_at_ns = Some(now.to_string());
        record.retry.last_result = RetryResult::Pending;
        persist(store, &record)?;
        let result = status(&record, &source, now);
        self.records.insert(source.id, record);
        Ok(result)
    }
}
