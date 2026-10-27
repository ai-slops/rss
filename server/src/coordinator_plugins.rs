use crate::{
    command::{Command, PluginCommand},
    coordinator::{Applied, Owner, now_ns},
    error::{AppError, Result},
    plugins::PluginError,
};

impl Owner {
    pub(crate) fn apply_plugin(
        &mut self,
        command: &Command,
        plugin_command: &PluginCommand,
    ) -> Result<Applied> {
        let snapshot = self.snapshot()?;
        let result = self
            .plugins
            .execute(&command.target_id.0, plugin_command, &snapshot, &self.store)
            .map_err(|error| match error {
                PluginError::Invalid(reason) => AppError::Invalid(reason),
                PluginError::Internal(error) => AppError::Internal(error),
            })?;
        Ok(Applied {
            revision: None,
            at_ns: now_ns(),
            plugin_result: Some(result),
        })
    }
}
