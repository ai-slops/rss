use crate::{
    error::{AppError, Result},
    source_recovery_types::PollRequest,
};

pub fn validate_poll(request: &PollRequest) -> Result<()> {
    if request.schema_version != 1 {
        return Err(AppError::Invalid(
            "Unsupported recovery schemaVersion.".into(),
        ));
    }
    uuid::Uuid::parse_str(&request.controller_id)
        .map_err(|_| AppError::Invalid("controllerId must be a UUID.".into()))?;
    for (name, value) in [
        ("processEpoch", &request.process_epoch),
        ("pollSeq", &request.poll_seq),
        ("intentEpoch", &request.intent_epoch),
    ] {
        parse_required(value, name)?;
    }
    if let Some(action) = &request.last_action {
        uuid::Uuid::parse_str(&action.command_id)
            .map_err(|_| AppError::Invalid("lastAction.commandId must be a UUID.".into()))?;
    }
    Ok(())
}

pub fn parse_required(value: &str, field: &str) -> Result<u64> {
    value.parse().map_err(|_| {
        AppError::Invalid(format!(
            "{field} must be a decimal-string nonnegative integer."
        ))
    })
}
