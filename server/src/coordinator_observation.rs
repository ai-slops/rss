use serde_json::Value;

use crate::{
    command::{Command, Receipt},
    coordinator::{Owner, now_ns},
    error::{AppError, Result},
    observation::EventPage,
};

impl Owner {
    pub(crate) fn command(&mut self, command: Command) -> Result<Value> {
        if let Some(receipt) = self
            .store
            .command(&command.id)
            .map_err(AppError::Internal)?
        {
            return Ok(receipt);
        }
        let accepted_at = now_ns();
        let result = self.apply(&command);
        let (status, applied_at, revision, reason, plugin_result) = match result {
            Ok(applied) => (
                "applied",
                Some(applied.at_ns),
                applied.revision,
                None,
                applied.plugin_result,
            ),
            Err(AppError::Invalid(reason) | AppError::Conflict(reason)) => {
                ("rejected", None, None, Some(reason), None)
            }
            Err(AppError::Unavailable(reason)) => ("failed", None, None, Some(reason), None),
            Err(AppError::Unauthorized) => (
                "rejected",
                None,
                None,
                Some("Unauthorized command.".into()),
                None,
            ),
            Err(AppError::Internal(_)) => (
                "failed",
                None,
                None,
                Some(
                    "The command outcome is uncertain. Refresh authoritative state before retrying."
                        .into(),
                ),
                None,
            ),
        };
        let receipt = Receipt {
            command_id: command.id.clone(),
            status,
            accepted_at,
            applied_at: applied_at.clone(),
            actual_revision: revision,
            reason,
            plugin_result,
        };
        let value =
            serde_json::to_value(receipt).map_err(|error| AppError::Internal(error.into()))?;
        self.append_event(
            &command.id,
            status,
            applied_at.unwrap_or_else(now_ns),
            value.clone(),
        )?;
        Ok(value)
    }

    fn append_event(
        &mut self,
        command_id: &str,
        kind: &str,
        at: String,
        receipt: Value,
    ) -> Result<()> {
        let mut snapshot = self.snapshot()?;
        let event = self
            .store
            .append_observation(
                command_id,
                self.media.engine_run_id(),
                &at,
                &format!("production.{kind}"),
                receipt,
                &mut snapshot,
            )
            .map_err(AppError::Internal)?;
        self.plugins.observe(&event, &snapshot, &self.store);
        let _ = self.events.send(());
        Ok(())
    }

    pub(crate) fn events(&self, after: &str) -> Result<EventPage> {
        self.store.events(after).map_err(AppError::Internal)
    }
}
