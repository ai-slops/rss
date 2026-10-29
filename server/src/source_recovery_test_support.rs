use super::*;

pub(super) fn source(id: &str) -> SourceContext {
    SourceContext {
        id: id.into(),
        enabled: true,
        srt: true,
    }
}

pub(super) fn poll(
    controller: &str,
    process: u64,
    sequence: u64,
    intent: u64,
    desired: LocalDesired,
) -> PollRequest {
    PollRequest {
        schema_version: 1,
        controller_id: controller.into(),
        process_epoch: process.to_string(),
        poll_seq: sequence.to_string(),
        intent_epoch: intent.to_string(),
        local_desired: desired,
        publisher_state: PublisherState::Sending,
        last_action: None,
    }
}
