use crate::{
    source_recovery::{HealthObservation, SourceContext, SourceRecoveryManager},
    source_recovery_types::{
        ActionOutcome, ActionResult, LocalDesired, PollRequest, PublisherState, RecoveryPolicy,
        RetryResult,
    },
    store::Store,
};

fn source() -> SourceContext {
    SourceContext {
        id: "source-budget".into(),
        enabled: true,
        srt: true,
    }
}
fn poll(controller: &str, sequence: u64) -> PollRequest {
    PollRequest {
        schema_version: 1,
        controller_id: controller.into(),
        process_epoch: "1".into(),
        poll_seq: sequence.to_string(),
        intent_epoch: if sequence == 1 { "1" } else { "2" }.into(),
        local_desired: LocalDesired::Running,
        publisher_state: PublisherState::Failed,
        last_action: None,
    }
}

#[test]
fn three_failed_actions_exhaust_persisted_budget() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("project.sqlite");
    let store = Store::open(&path)?;
    let mut manager = SourceRecoveryManager::load(&store)?;
    let (enrollment, _) = manager.enroll(&store, source(), false, 0)?;
    manager.set_policy(
        &store,
        source(),
        "1",
        RecoveryPolicy {
            enabled: true,
            maintenance: false,
        },
        0,
    )?;
    manager.poll(
        &store,
        source(),
        poll(&enrollment.controller_id, 1),
        "run",
        1,
    )?;
    manager.poll(
        &store,
        source(),
        poll(&enrollment.controller_id, 2),
        "run",
        2,
    )?;
    manager.observe(
        &store,
        source(),
        HealthObservation {
            generation: "1".into(),
            video_frames: "1".into(),
            audio_buffers: "1".into(),
            last_video_pts_ns: Some("1".into()),
        },
        2,
    )?;
    manager.observe(
        &store,
        source(),
        HealthObservation {
            generation: "1".into(),
            video_frames: "1".into(),
            audio_buffers: "1".into(),
            last_video_pts_ns: Some("1".into()),
        },
        3_000_000_003,
    )?;
    let mut now = 3_000_000_004_u64;
    let mut sequence = 3_u64;
    for expected in 1..=3 {
        let action = manager
            .poll(
                &store,
                source(),
                poll(&enrollment.controller_id, sequence),
                "run",
                now,
            )?
            .action
            .expect("action");
        assert_eq!(action.attempt, expected);
        sequence += 1;
        let mut acknowledgement = poll(&enrollment.controller_id, sequence);
        acknowledgement.last_action = Some(ActionResult {
            command_id: action.command_id,
            result: ActionOutcome::Failed,
        });
        manager.poll(&store, source(), acknowledgement, "run", now + 1)?;
        now += (1_u64 << u32::from(expected - 1)) * 1_000_000_000;
        sequence += 1;
    }
    let exhausted = manager.poll(
        &store,
        source(),
        poll(&enrollment.controller_id, sequence),
        "run",
        now,
    )?;
    assert!(exhausted.action.is_none());
    assert_eq!(exhausted.status.retry.last_result, RetryResult::Exhausted);
    drop(manager);
    let mut restored = SourceRecoveryManager::load(&Store::open(&path)?)?;
    assert_eq!(restored.status(source(), now).retry.attempts, 3);
    Ok(())
}
