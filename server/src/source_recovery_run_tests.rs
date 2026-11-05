use super::{
    test_support::{poll, source},
    *,
};
use crate::source_recovery_types::{ActionOutcome, ActionResult};

#[test]
fn an_old_server_action_ack_cannot_become_recovered_in_a_new_run() -> anyhow::Result<()> {
    // Given: an authenticated controller has one pending action in the old engine run.
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("project.sqlite");
    let store = Store::open(&path)?;
    let mut manager = SourceRecoveryManager::load(&store)?;
    let (enrollment, _) = manager.enroll(&store, source("source-a"), false, 0)?;
    manager.set_policy(
        &store,
        source("source-a"),
        "1",
        RecoveryPolicy {
            enabled: true,
            maintenance: false,
        },
        0,
    )?;
    manager.poll(
        &store,
        source("source-a"),
        poll(&enrollment.controller_id, 1, 1, 1, LocalDesired::Stopped),
        "old-run",
        0,
    )?;
    manager.poll(
        &store,
        source("source-a"),
        poll(&enrollment.controller_id, 1, 2, 2, LocalDesired::Running),
        "old-run",
        1,
    )?;
    let record = manager.records.get_mut("source-a").expect("source record");
    record.receiver.state = ReceiverState::Stalled;
    record.receiver.continuity_generation = Some("1".into());
    record.receiver.video_frames = Some("10".into());
    record.receiver.audio_buffers = Some("10".into());
    record.receiver.last_video_pts_ns = Some("10".into());
    let issued = manager.poll(
        &store,
        source("source-a"),
        poll(&enrollment.controller_id, 1, 3, 2, LocalDesired::Running),
        "old-run",
        3_100_000_000,
    )?;
    let old_action = issued.action.expect("pending recovery action");
    drop(manager);

    // When: a new daemon loads that record and receives an acknowledgement of the old action.
    let mut restored = SourceRecoveryManager::load(&Store::open(&path)?)?;
    let old_duplicate = restored.poll(
        &store,
        source("source-a"),
        poll(&enrollment.controller_id, 1, 3, 2, LocalDesired::Running),
        "new-run",
        4_000_000_000,
    );
    assert!(matches!(old_duplicate, Err(AppError::Conflict(_))));
    let mut acknowledgement = poll(&enrollment.controller_id, 1, 4, 2, LocalDesired::Running);
    acknowledgement.last_action = Some(ActionResult {
        command_id: old_action.command_id,
        result: ActionOutcome::Started,
    });
    let response = restored.poll(
        &store,
        source("source-a"),
        acknowledgement,
        "new-run",
        4_100_000_000,
    )?;

    // Then: run-local generation numbers cannot validate an action from the previous engine.
    assert_eq!(response.status.retry.last_result, RetryResult::Cancelled);
    assert_eq!(response.status.retry.attempts, 1);
    assert!(response.action.is_none());
    assert_eq!(response.status.receiver.state, ReceiverState::Unknown);
    for (value, at) in [
        ("1", 4_200_000_000),
        ("2", 4_300_000_000),
        ("3", 4_400_000_000),
    ] {
        restored.observe(
            &store,
            source("source-a"),
            HealthObservation {
                generation: "2".into(),
                video_frames: value.into(),
                audio_buffers: value.into(),
                last_video_pts_ns: Some(value.into()),
            },
            at,
        )?;
    }
    let final_status = restored.status(source("source-a"), 4_400_000_000);
    assert_eq!(final_status.retry.last_result, RetryResult::Cancelled);
    assert_eq!(final_status.retry.attempts, 1);
    Ok(())
}
