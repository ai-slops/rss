use std::collections::HashMap;

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::{
    error::{AppError, Result},
    source_recovery_attempt::maybe_action,
    source_recovery_duplicate::{PollPreflight, cache, preflight},
    source_recovery_health::apply_observation,
    source_recovery_state::{apply_poll, persist, status},
    source_recovery_types::*,
    source_recovery_validation::validate_poll,
    store::Store,
};

pub(crate) const NAMESPACE: &str = "source-recovery.v1";
pub(crate) const LEASE_NS: u64 = 5_000_000_000;
pub(crate) const MAXIMUM_ATTEMPTS: u8 = 3;
const MAXIMUM_REQUEST_IDS: usize = 64;

pub struct SourceRecoveryManager {
    records: HashMap<String, RecoveryRecord>,
    bound_run_id: Option<String>,
}

pub struct SourceContext {
    pub id: String,
    pub enabled: bool,
    pub srt: bool,
}

pub struct HealthObservation {
    pub generation: String,
    pub video_frames: String,
    pub audio_buffers: String,
    pub last_video_pts_ns: Option<String>,
}

pub struct Enrollment {
    pub controller_id: String,
    pub controller_token: String,
}

impl SourceRecoveryManager {
    pub fn load(store: &Store) -> anyhow::Result<Self> {
        let records = store
            .plugin_list(NAMESPACE)?
            .into_iter()
            .map(|(key, value)| Ok((key, serde_json::from_value(value)?)))
            .collect::<anyhow::Result<HashMap<_, _>>>()?;
        Ok(Self {
            records,
            bound_run_id: None,
        })
    }

    pub fn status(&mut self, source: SourceContext, now: u64) -> RecoveryStatus {
        let record = self.record_for_source(&source.id);
        status(&record, &source, now)
    }

    pub fn authenticate(&self, source_id: &str, token: &str) -> Result<()> {
        let candidate: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        let stored = self
            .records
            .get(source_id)
            .and_then(|record| record.controller.as_ref())
            .and_then(|controller| decode_hash(&controller.token_hash));
        if stored.is_some_and(|value| bool::from(value.ct_eq(&candidate))) {
            Ok(())
        } else {
            Err(AppError::Unauthorized)
        }
    }

    pub fn poll(
        &mut self,
        store: &Store,
        source: SourceContext,
        request: PollRequest,
        server_run_id: &str,
        now: u64,
    ) -> Result<PollResponse> {
        validate_poll(&request)?;
        self.bind_run(store, server_run_id)?;
        if let PollPreflight::Duplicate(response) =
            self.preflight_poll(&source, &request, server_run_id, now)?
        {
            return Ok(*response);
        }
        let mut record = self.record_for_source(&source.id);
        apply_poll(&mut record, &request, now)?;
        let action = maybe_action(&mut record, &source, server_run_id, now);
        let response = PollResponse {
            status: status(&record, &source, now),
            server_run_id: server_run_id.to_owned(),
            process_epoch: request.process_epoch.clone(),
            poll_seq: request.poll_seq.clone(),
            intent_epoch: request.intent_epoch.clone(),
            next_poll_ms: 1000,
            action,
        };
        cache(&mut record, &response);
        persist(store, &record)?;
        self.records.insert(source.id, record);
        Ok(response)
    }

    pub fn preflight_poll(
        &self,
        source: &SourceContext,
        request: &PollRequest,
        server_run_id: &str,
        now: u64,
    ) -> Result<PollPreflight> {
        if self.bound_run_id.as_deref() != Some(server_run_id) {
            return Err(AppError::Conflict("Recovery engine run is stale.".into()));
        }
        preflight(
            self.records.get(&source.id).ok_or(AppError::Unauthorized)?,
            request,
            source.enabled,
            server_run_id,
            now,
        )
    }

    pub fn observe(
        &mut self,
        store: &Store,
        source: SourceContext,
        observation: HealthObservation,
        now: u64,
    ) -> Result<()> {
        let mut record = self.record_for_source(&source.id);
        apply_observation(&mut record, &source, observation, now);
        persist(store, &record)?;
        self.records.insert(source.id, record);
        Ok(())
    }

    fn record_for_source(&self, source_id: &str) -> RecoveryRecord {
        self.records.get(source_id).cloned().unwrap_or_else(|| {
            let mut record = RecoveryRecord::new(source_id);
            record.server_run_id.clone_from(&self.bound_run_id);
            record
        })
    }
}

#[path = "source_recovery_scope.rs"]
mod scope;

#[path = "source_recovery_operator.rs"]
mod operator;
#[cfg(test)]
#[path = "source_recovery_run_tests.rs"]
mod run_tests;
#[cfg(test)]
#[path = "source_recovery_test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "source_recovery_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "source_recovery_transition_tests.rs"]
mod transition_tests;

fn hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn decode_hash(value: &str) -> Option<[u8; 32]> {
    let bytes = (0..64)
        .step_by(2)
        .map(|index| u8::from_str_radix(value.get(index..index + 2)?, 16).ok())
        .collect::<Option<Vec<_>>>()?;
    bytes.try_into().ok()
}
