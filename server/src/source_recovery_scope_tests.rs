use super::super::{
    HealthObservation,
    test_support::{poll, source},
};
use super::*;
use crate::source_recovery_types::{LocalDesired, ReceiverState};

#[test]
fn startup_rebases_started_health_and_preserves_the_remaining_budget() -> anyhow::Result<()> {
    // Given
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("project.sqlite");
    let store = Store::open(&path)?;
    let mut manager = SourceRecoveryManager::load_for_run(&store, "old-run")?;
    let (enrollment, _) = manager.enroll(&store, source("source-a"), false, 0)?;
    let record = manager.records.get_mut("source-a").expect("record");
    record.policy.enabled = true;
    record.retry.attempts = 2;
    record.retry.last_result = RetryResult::Started;
    record.retry.baseline_generation = Some("1".into());
    record.retry.healthy_observations = 1;
    record.retry.next_eligible_at_ns = Some("1000".into());
    record.retry.request_ids = vec!["manual-request".into()];
    record.receiver.state = ReceiverState::Healthy;
    record.receiver.continuity_generation = Some("1".into());
    record.receiver.video_frames = Some("10".into());
    record.receiver.audio_buffers = Some("10".into());
    record.receiver.last_video_pts_ns = Some("10".into());
    let controller = record.controller.as_mut().expect("controller");
    controller.process_epoch = "4".into();
    controller.poll_seq = "9".into();
    controller.intent_epoch = "7".into();
    controller.local_desired = LocalDesired::Running;
    controller.armed = true;
    controller.last_poll_ns = Some("100".into());
    store.plugin_put(NAMESPACE, "source-a", &serde_json::to_value(record)?)?;

    // When
    let mut restored = SourceRecoveryManager::load_for_run(&Store::open(&path)?, "new-run")?;
    let status = restored.status(source("source-a"), 200);
    assert_eq!(status.receiver.continuity_generation, None);
    assert!(!status.controller.online);
    assert_eq!(status.controller.process_epoch.as_deref(), Some("4"));
    assert_eq!(status.controller.local_desired, "running");
    restored.authenticate("source-a", &enrollment.controller_token)?;
    for (value, at) in [("1", 300), ("2", 400), ("3", 500)] {
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

    // Then
    assert_eq!(
        restored.status(source("source-a"), 500).retry.last_result,
        RetryResult::Cancelled
    );
    assert_eq!(restored.records["source-a"].retry.attempts, 2);
    assert_eq!(
        restored.records["source-a"].retry.request_ids,
        vec!["manual-request"]
    );
    restored.poll(
        &store,
        source("source-a"),
        poll(&enrollment.controller_id, 4, 10, 7, LocalDesired::Running),
        "new-run",
        1000,
    )?;
    restored.observe(
        &store,
        source("source-a"),
        HealthObservation {
            generation: "2".into(),
            video_frames: "3".into(),
            audio_buffers: "3".into(),
            last_video_pts_ns: Some("3".into()),
        },
        3_000_001_000,
    )?;
    let next = restored.poll(
        &store,
        source("source-a"),
        poll(&enrollment.controller_id, 4, 11, 7, LocalDesired::Running),
        "new-run",
        3_000_001_001,
    )?;
    assert_eq!(next.action.expect("remaining attempt").attempt, 3);
    Ok(())
}

#[test]
fn failed_batch_binding_rolls_back_database_and_live_manager() -> anyhow::Result<()> {
    // Given
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("project.sqlite");
    let store = Store::open(&path)?;
    let mut manager = SourceRecoveryManager::load_for_run(&store, "old-run")?;
    manager.enroll(&store, source("source-a"), false, 0)?;
    manager.enroll(&store, source("source-b"), false, 0)?;
    let before_a = store.plugin_get(NAMESPACE, "source-a")?;
    let before_b = store.plugin_get(NAMESPACE, "source-b")?;
    store.db.execute_batch("CREATE TRIGGER fail_second_scope BEFORE UPDATE ON plugin_storage WHEN NEW.namespace='source-recovery.v1' AND NEW.key='source-b' BEGIN SELECT RAISE(ABORT,'scope write failed'); END;")?;

    // When
    assert!(manager.bind_run(&store, "new-run").is_err());

    // Then
    assert_eq!(manager.bound_run_id.as_deref(), Some("old-run"));
    assert_eq!(
        manager.records["source-a"].server_run_id.as_deref(),
        Some("old-run")
    );
    assert_eq!(store.plugin_get(NAMESPACE, "source-a")?, before_a);
    assert_eq!(store.plugin_get(NAMESPACE, "source-b")?, before_b);
    let restored = SourceRecoveryManager::load(&Store::open(&path)?)?;
    assert_eq!(
        restored.records["source-a"].server_run_id.as_deref(),
        Some("old-run")
    );
    Ok(())
}

#[test]
fn legacy_binding_preserves_queued_manual_retry_without_reusing_old_health() -> anyhow::Result<()> {
    // Given
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("project.sqlite"))?;
    let mut manager = SourceRecoveryManager::load(&store)?;
    manager.enroll(&store, source("source-a"), false, 0)?;
    let record = manager.records.get_mut("source-a").expect("record");
    record.retry.last_result = RetryResult::Pending;
    record.retry.attempts = 2;
    record.retry.next_eligible_at_ns = Some("9000000000".into());
    record.retry.request_ids = vec!["manual-request".into()];
    record.receiver.state = ReceiverState::Healthy;
    let mut legacy = serde_json::to_value(record)?;
    legacy
        .as_object_mut()
        .expect("record object")
        .remove("serverRunId");
    store.plugin_put(NAMESPACE, "source-a", &legacy)?;

    // When
    let restored = SourceRecoveryManager::load_for_run(&store, "new-run")?;

    // Then
    let rebound = &restored.records["source-a"];
    assert_eq!(rebound.server_run_id.as_deref(), Some("new-run"));
    assert_eq!(rebound.receiver.state, ReceiverState::Unknown);
    assert_eq!(rebound.retry.last_result, RetryResult::Pending);
    assert_eq!(rebound.retry.attempts, 2);
    assert_eq!(
        rebound.retry.next_eligible_at_ns.as_deref(),
        Some("9000000000")
    );
    assert_eq!(rebound.retry.request_ids, vec!["manual-request"]);
    Ok(())
}
