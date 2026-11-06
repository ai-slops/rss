use super::{NAMESPACE, SourceRecoveryManager};
use crate::{
    error::{AppError, Result},
    source_recovery_state::cancel,
    source_recovery_types::{PublisherState, ReceiverRecord, RetryResult},
    store::Store,
};

impl SourceRecoveryManager {
    pub fn load_for_run(store: &Store, run_id: &str) -> Result<Self> {
        let mut manager = Self::load(store).map_err(AppError::Internal)?;
        manager.bind_run(store, run_id)?;
        Ok(manager)
    }

    pub fn bind_run(&mut self, store: &Store, run_id: &str) -> Result<()> {
        if self.bound_run_id.as_deref() == Some(run_id) {
            return Ok(());
        }
        let mut rebound = self.records.clone();
        for record in rebound.values_mut() {
            if record.server_run_id.as_deref() == Some(run_id) {
                continue;
            }
            if record.retry.in_flight.is_some() || record.retry.last_result == RetryResult::Started
            {
                cancel(record);
            }
            record.receiver = ReceiverRecord::default();
            record.retry.in_flight = None;
            record.retry.healthy_observations = 0;
            record.retry.baseline_generation = None;
            record.retry.action_grace_started_ns = None;
            if let Some(controller) = record.controller.as_mut() {
                controller.last_poll_ns = None;
                controller.last_poll_fingerprint = None;
                controller.cached_response = None;
                controller.publisher_state = PublisherState::Stopped;
            }
            record.server_run_id = Some(run_id.to_owned());
        }
        let mut rows = rebound
            .iter()
            .map(|(id, record)| serde_json::to_value(record).map(|value| (id.clone(), value)))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| AppError::Internal(error.into()))?;
        rows.sort_by(|left, right| left.0.cmp(&right.0));
        store
            .plugin_put_batch(NAMESPACE, &rows)
            .map_err(AppError::Internal)?;
        self.records = rebound;
        self.bound_run_id = Some(run_id.to_owned());
        Ok(())
    }
}

#[cfg(test)]
#[path = "source_recovery_scope_tests.rs"]
mod tests;
