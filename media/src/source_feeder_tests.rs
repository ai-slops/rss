use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use super::*;
use crate::{
    output_pipeline::source_recording_pipeline, source_capture::SourceCaptureRegistry,
    source_feeder_limits::limit_reached, tap::TapChannels,
};

#[test]
fn second_spawn_failure_joins_first_worker_after_closing_subscription() -> anyhow::Result<()> {
    // Given
    let fixture = fixture()?;
    let joined = Arc::new(AtomicUsize::new(0));
    let disconnected = Arc::new(AtomicUsize::new(0));
    let mut spawn_count = 0_u8;
    let started = Instant::now();

    // When
    let result = SourceFeeders::start_with_spawner(fixture.context(), |_, subscription, _| {
        spawn_count = spawn_count.saturating_add(1);
        if spawn_count == 2 {
            return Err("injected second spawn failure".to_owned());
        }
        observation_worker(subscription, &joined, &disconnected)
    });

    // Then
    assert_eq!(
        result.err().as_deref(),
        Some("injected second spawn failure")
    );
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(joined.load(Ordering::Acquire), 1);
    assert_eq!(disconnected.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn drop_closes_subscriptions_and_joins_both_workers() -> anyhow::Result<()> {
    // Given
    let fixture = fixture()?;
    let joined = Arc::new(AtomicUsize::new(0));
    let disconnected = Arc::new(AtomicUsize::new(0));
    let feeders = SourceFeeders::start_with_spawner(fixture.context(), |_, subscription, _| {
        observation_worker(subscription, &joined, &disconnected)
    })
    .map_err(anyhow::Error::msg)?;

    // When
    drop(feeders);

    // Then
    assert_eq!(joined.load(Ordering::Acquire), 2);
    assert_eq!(disconnected.load(Ordering::Acquire), 2);
    Ok(())
}

#[test]
fn source_appsrc_and_downstream_queues_keep_fixed_buffer_bounds() -> anyhow::Result<()> {
    // Given
    let fixture = fixture()?;

    // When
    let video =
        app_source(&fixture.pipeline, "recording_video_input").map_err(anyhow::Error::msg)?;
    let audio =
        app_source(&fixture.pipeline, "recording_audio_input").map_err(anyhow::Error::msg)?;
    let mut queues = fixture
        .pipeline
        .iterate_elements()
        .into_iter()
        .filter_map(Result::ok)
        .filter(|element| {
            element
                .factory()
                .is_some_and(|factory| factory.name() == "queue")
        })
        .map(|queue| queue.property::<u32>("max-size-buffers"))
        .collect::<Vec<_>>();
    queues.sort_unstable();

    // Then
    assert!(video.property::<bool>("is-live"));
    assert!(!video.property::<bool>("block"));
    assert_eq!(video.property::<u64>("max-buffers"), 8);
    assert_eq!(video.property::<u64>("max-bytes"), 0);
    assert_eq!(video.property::<u64>("max-time"), 0);
    assert!(audio.property::<bool>("is-live"));
    assert!(!audio.property::<bool>("block"));
    assert_eq!(audio.property::<u64>("max-buffers"), 64);
    assert_eq!(audio.property::<u64>("max-bytes"), 0);
    assert_eq!(audio.property::<u64>("max-time"), 0);
    assert_eq!(queues, vec![8, 64]);
    assert!(!limit_reached(7, 8));
    assert!(limit_reached(8, 8));
    assert!(!limit_reached(63, 64));
    assert!(limit_reached(64, 64));
    Ok(())
}

#[test]
fn stop_observes_a_sticky_fault_published_while_workers_are_joining() -> anyhow::Result<()> {
    let fixture = fixture()?;
    let feeders = SourceFeeders::start_with_spawner(fixture.context(), |_, subscription, _| {
        let receiver = subscription.receiver.take().expect("receiver");
        let fault = Arc::clone(&subscription.fault);
        thread::Builder::new()
            .spawn(move || {
                assert!(receiver.recv().is_err());
                *fault.lock().expect("fault") = Some(SourceCaptureFault::InvalidCapture);
            })
            .map_err(|error| error.to_string())
    })
    .map_err(anyhow::Error::msg)?;

    assert_eq!(
        feeders.stop().err().as_deref(),
        Some("source capture omitted required provenance")
    );
    Ok(())
}

struct Fixture {
    _directory: tempfile::TempDir,
    pipeline: gst::Pipeline,
    capture: crate::source_capture::SourceCapture,
    provenance: Arc<ProvenanceCollector>,
}

impl Fixture {
    fn context(&self) -> StartContext<'_> {
        StartContext {
            pipeline: &self.pipeline,
            capture: &self.capture,
            media_origin_ns: 0,
            provenance: &self.provenance,
        }
    }
}

fn fixture() -> anyhow::Result<Fixture> {
    gst::init()?;
    let directory = tempfile::tempdir()?;
    let pattern = directory.path().join("segment-%05d.mkv");
    let pipeline = source_recording_pipeline(
        pattern.to_str().ok_or_else(|| anyhow::anyhow!("pattern"))?,
        Duration::from_secs(1),
    )?;
    let capture = SourceCaptureRegistry::default().capture(&TapChannels {
        video: "feeder-test-video".to_owned(),
        audio: "feeder-test-audio".to_owned(),
    });
    Ok(Fixture {
        _directory: directory,
        pipeline,
        capture,
        provenance: Arc::new(ProvenanceCollector::default()),
    })
}

fn observation_worker(
    subscription: &mut TrackSubscription,
    joined: &Arc<AtomicUsize>,
    disconnected: &Arc<AtomicUsize>,
) -> Result<JoinHandle<()>, String> {
    let receiver = subscription
        .receiver
        .take()
        .ok_or_else(|| "missing receiver".to_owned())?;
    let joined = Arc::clone(joined);
    let disconnected = Arc::clone(disconnected);
    thread::Builder::new()
        .spawn(move || {
            let closed = matches!(
                receiver.recv_timeout(Duration::from_millis(750)),
                Err(mpsc::RecvTimeoutError::Disconnected)
            );
            if closed {
                disconnected.fetch_add(1, Ordering::AcqRel);
            }
            joined.fetch_add(1, Ordering::AcqRel);
        })
        .map_err(|error| error.to_string())
}
