use super::{
    test_support::{poll, source},
    *,
};
use crate::source_recovery_types::{ActionOutcome, ActionResult};

fn armed(store: &Store) -> anyhow::Result<(SourceRecoveryManager, Enrollment)> {
    let mut manager = SourceRecoveryManager::load(store)?;
    let (enrollment, _) = manager.enroll(store, source("source-a"), false, 0)?;
    manager.set_policy(
        store,
        source("source-a"),
        "1",
        RecoveryPolicy {
            enabled: true,
            maintenance: false,
        },
        0,
    )?;
    manager.poll(
        store,
        source("source-a"),
        poll(&enrollment.controller_id, 1, 1, 1, LocalDesired::Stopped),
        "run",
        0,
    )?;
    manager.poll(
        store,
        source("source-a"),
        poll(&enrollment.controller_id, 1, 2, 2, LocalDesired::Running),
        "run",
        1,
    )?;
    Ok((manager, enrollment))
}

#[test]
fn direct_duplicate_poll_cannot_create_a_new_action() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("project.sqlite"))?;
    let (mut manager, enrollment) = armed(&store)?;
    manager
        .records
        .get_mut("source-a")
        .expect("record")
        .receiver
        .state = ReceiverState::Stalled;
    let duplicate = manager.poll(
        &store,
        source("source-a"),
        poll(&enrollment.controller_id, 1, 2, 2, LocalDesired::Running),
        "run",
        4_000_000_000,
    )?;
    assert!(duplicate.action.is_none());
    assert_eq!(
        manager
            .status(source("source-a"), 4_000_000_000)
            .retry
            .attempts,
        0
    );
    Ok(())
}

#[test]
fn started_action_has_decode_grace_and_requires_new_generation() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("project.sqlite"))?;
    let (mut manager, enrollment) = armed(&store)?;
    let record = manager.records.get_mut("source-a").expect("record");
    record.receiver.state = ReceiverState::Stalled;
    record.receiver.continuity_generation = Some("1".into());
    record.receiver.video_frames = Some("10".into());
    record.receiver.audio_buffers = Some("10".into());
    record.receiver.last_video_pts_ns = Some("10".into());
    record.retry.request_ids.push("manual-request".into());
    let issued = manager.poll(
        &store,
        source("source-a"),
        poll(&enrollment.controller_id, 1, 3, 2, LocalDesired::Running),
        "run",
        3_100_000_000,
    )?;
    let mut ack = poll(&enrollment.controller_id, 1, 4, 2, LocalDesired::Running);
    ack.last_action = Some(ActionResult {
        command_id: issued.action.expect("action").command_id,
        result: ActionOutcome::Started,
    });
    let accepted = manager.poll(&store, source("source-a"), ack, "run", 4_100_000_000)?;
    assert!(
        accepted.action.is_none(),
        "a start acknowledgement must not spend another attempt"
    );
    assert_eq!(accepted.status.retry.attempts, 1);
    for (generation, value, at) in [("1", "11", 4_200_000_000), ("1", "12", 4_300_000_000)] {
        manager.observe(
            &store,
            source("source-a"),
            HealthObservation {
                generation: generation.into(),
                video_frames: value.into(),
                audio_buffers: value.into(),
                last_video_pts_ns: Some(value.into()),
            },
            at,
        )?;
    }
    assert_eq!(
        manager
            .status(source("source-a"), 4_300_000_000)
            .retry
            .last_result,
        RetryResult::Started
    );
    for (value, at) in [
        ("1", 4_400_000_000),
        ("2", 4_500_000_000),
        ("3", 4_600_000_000),
    ] {
        manager.observe(
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
    assert_eq!(
        manager
            .status(source("source-a"), 4_600_000_000)
            .retry
            .last_result,
        RetryResult::Recovered
    );
    assert_eq!(
        manager.records["source-a"].retry.request_ids,
        vec!["manual-request"]
    );
    Ok(())
}
