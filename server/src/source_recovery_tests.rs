use super::test_support::{poll, source};
use super::*;

#[test]
fn restart_is_fenced_deduplicated_cancelled_and_persisted() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("project.sqlite");
    let store = Store::open(&path)?;
    let mut manager = SourceRecoveryManager::load(&store)?;
    let (enrollment, _) = manager.enroll(&store, source("source-a"), false, 0)?;
    assert!(
        !store
            .plugin_get(NAMESPACE, "source-a")?
            .expect("record")
            .to_string()
            .contains(&enrollment.controller_token)
    );
    let policy = manager.set_policy(
        &store,
        source("source-a"),
        "1",
        RecoveryPolicy {
            enabled: true,
            maintenance: false,
        },
        0,
    )?;
    assert_eq!(policy.policy_version, "2");
    assert!(matches!(
        manager.set_policy(
            &store,
            source("source-a"),
            "1",
            RecoveryPolicy {
                enabled: false,
                maintenance: false
            },
            1
        ),
        Err(AppError::Conflict(_))
    ));
    let first = manager.poll(
        &store,
        source("source-a"),
        poll(&enrollment.controller_id, 1, 1, 1, LocalDesired::Running),
        "run-a",
        1,
    )?;
    assert!(first.action.is_none());
    assert_eq!(first.status.controller.local_desired, "stopped");
    manager.poll(
        &store,
        source("source-a"),
        poll(&enrollment.controller_id, 1, 2, 2, LocalDesired::Running),
        "run-a",
        2,
    )?;
    manager.observe(
        &store,
        source("source-a"),
        HealthObservation {
            generation: "1".into(),
            video_frames: "10".into(),
            audio_buffers: "10".into(),
            last_video_pts_ns: Some("100".into()),
        },
        2,
    )?;
    manager.observe(
        &store,
        source("source-a"),
        HealthObservation {
            generation: "1".into(),
            video_frames: "10".into(),
            audio_buffers: "10".into(),
            last_video_pts_ns: Some("100".into()),
        },
        3_000_000_003,
    )?;
    let issued = manager.poll(
        &store,
        source("source-a"),
        poll(&enrollment.controller_id, 1, 3, 2, LocalDesired::Running),
        "run-a",
        3_000_000_004,
    )?;
    let duplicate = manager.poll(
        &store,
        source("source-a"),
        poll(&enrollment.controller_id, 1, 3, 2, LocalDesired::Running),
        "run-a",
        3_000_000_005,
    )?;
    let stopped = manager.poll(
        &store,
        source("source-a"),
        poll(&enrollment.controller_id, 1, 4, 3, LocalDesired::Stopped),
        "run-a",
        3_000_000_006,
    )?;
    assert_eq!(
        issued.action.as_ref().map(|v| &v.command_id),
        duplicate.action.as_ref().map(|v| &v.command_id)
    );
    assert_eq!(issued.status.retry.attempts, 1);
    assert!(stopped.action.is_none());
    assert_eq!(stopped.status.retry.last_result, RetryResult::Cancelled);
    assert!(
        manager
            .authenticate("source-b", &enrollment.controller_token)
            .is_err()
    );
    let mut restored = SourceRecoveryManager::load(&Store::open(&path)?)?;
    assert_eq!(
        restored
            .status(source("source-a"), 3_000_000_007)
            .retry
            .last_result,
        RetryResult::Cancelled
    );
    Ok(())
}

#[test]
fn higher_process_epoch_requires_newer_running_intent() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("project.sqlite"))?;
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
    let first = manager.poll(
        &store,
        source("source-a"),
        poll(&enrollment.controller_id, 2, 1, 8, LocalDesired::Running),
        "run-a",
        1,
    )?;
    assert_eq!(first.status.controller.local_desired, "stopped");
    let armed = manager.poll(
        &store,
        source("source-a"),
        poll(&enrollment.controller_id, 2, 2, 9, LocalDesired::Running),
        "run-a",
        2,
    )?;
    assert_eq!(armed.status.controller.local_desired, "running");
    let changed = manager.poll(
        &store,
        source("source-a"),
        poll(&enrollment.controller_id, 2, 2, 10, LocalDesired::Stopped),
        "run-a",
        3,
    );
    assert!(matches!(changed, Err(AppError::Conflict(_))));
    let observed = manager.status(source("source-a"), 3);
    assert_eq!(observed.controller.local_desired, "running");
    Ok(())
}

#[test]
fn failed_sqlite_writes_never_publish_live_transitions() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("project.sqlite");
    let store = Store::open(&path)?;
    let mut manager = SourceRecoveryManager::load(&store)?;
    let (enrollment, _) = manager.enroll(&store, source("source-a"), false, 0)?;
    let policy = manager.set_policy(
        &store,
        source("source-a"),
        "1",
        RecoveryPolicy {
            enabled: false,
            maintenance: false,
        },
        0,
    )?;
    assert_eq!(policy.policy_version, "2");
    store.db.execute_batch("CREATE TRIGGER fail_recovery BEFORE INSERT ON plugin_storage WHEN NEW.namespace='source-recovery.v1' BEGIN SELECT RAISE(ABORT,'fail'); END; CREATE TRIGGER fail_recovery_update BEFORE UPDATE ON plugin_storage WHEN NEW.namespace='source-recovery.v1' BEGIN SELECT RAISE(ABORT,'fail'); END;")?;
    assert!(
        manager
            .set_policy(
                &store,
                source("source-a"),
                "2",
                RecoveryPolicy {
                    enabled: true,
                    maintenance: false
                },
                1
            )
            .is_err()
    );
    assert!(!manager.status(source("source-a"), 1).policy.enabled);
    assert!(
        manager
            .enroll(&store, source("source-b"), false, 1)
            .is_err()
    );
    assert!(!manager.status(source("source-b"), 1).controller.enrolled);
    let request = poll(&enrollment.controller_id, 1, 1, 1, LocalDesired::Stopped);
    assert!(
        manager
            .poll(&store, source("source-a"), request, "run", 1)
            .is_err()
    );
    let process = manager
        .status(source("source-a"), 1)
        .controller
        .process_epoch;
    assert!(process.is_none());
    drop(manager);
    let mut restored = SourceRecoveryManager::load(&Store::open(&path)?)?;
    let restored_status = restored.status(source("source-a"), 1);
    assert_eq!(restored_status.policy_version, "2");
    assert!(!restored_status.policy.enabled);
    assert!(restored_status.controller.process_epoch.is_none());
    Ok(())
}
