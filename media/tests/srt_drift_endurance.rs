mod drift_support;
mod native_evidence;

use std::time::{Duration, Instant};

use drift_support::{Fixture, RuntimeSample, decode_recordings, rss_kib, sender};
use live_production_media::protocol::{SourceSnapshot, TrackDriftReport};

#[test]
fn sustained_independent_clocks_report_drift_and_preserve_bounded_progress() -> anyhow::Result<()> {
    // Given
    let fixture = Fixture::new()?;
    let started = Instant::now();
    let first_sender = sender(fixture.port, 1_500, 0)?;
    first_sender.start()?;
    let mut samples = RunSamples::default();

    // When
    let first_snapshot = run_phase(&fixture, Duration::from_secs(168), &mut samples)?;
    first_sender.stop()?;
    std::thread::sleep(Duration::from_secs(3));
    let disconnected = fixture.snapshot("drift-disconnected")?;
    let reconnect = sender(fixture.port, 0, -12_000)?;
    reconnect.start()?;
    let final_snapshot = run_reconnect_phase(&fixture, &mut samples)?;
    reconnect.stop()?;
    let audio_route_sequence = fixture.audio_route_sequence;
    let segments = fixture.finish()?;
    native_evidence::preserve(fixture.recording_directory(), "srt-drift")?;
    println!(
        "drift_before_decode duration_s={} frames={} rss_growth_kib={} max_video_queue_ns={} max_audio_queue_ns={} segments={}",
        started.elapsed().as_secs(),
        samples.program_frame_growth,
        samples.rss_growth_kib(),
        samples.max_video_queue_ns,
        samples.max_audio_queue_ns,
        segments.len()
    );
    let media = decode_recordings(&segments)?;

    // Then
    let video = report(&final_snapshot.source, true)?;
    let audio = report(&final_snapshot.source, false)?;
    let first_video = report(&first_snapshot.source, true)?;
    let first_audio = report(&first_snapshot.source, false)?;
    let encoded_video = serde_json::to_value(video)?;
    assert!(encoded_video["arrivalSkewNs"].is_string());
    assert!(encoded_video["observationWindowNanoseconds"].is_string());
    assert!(encoded_video["timestampResidualRms"].is_string());
    assert!(encoded_video["rateStandardErrorPpm"].is_number());
    assert!(encoded_video.get("rateUncertaintyPpm").is_none());
    assert!(started.elapsed() >= Duration::from_secs(180));
    println!(
        "drift_evidence duration_s={} frames={} audio_route_applied_sequence={} recorded_nonblack_pixels={} recorded_pcm_rms={:.2} injected_first_ppm=1500 first_video_ppm={} first_audio_ppm={} injected_reconnect_ppm=-12000 reconnect_video_ppm={} reconnect_video_skew_ns={} reconnect_video_window_ns={} reconnect_video_standard_error_ppm={} reconnect_video_residual_rms_ns={} reconnect_audio_ppm={} reconnect_audio_skew_ns={} reconnect_audio_window_ns={} reconnect_audio_standard_error_ppm={} reconnect_audio_residual_rms_ns={} timing_changes=3 max_video_queue_ns={} max_audio_queue_ns={} rss_growth_kib={} segments={}",
        started.elapsed().as_secs(),
        samples.program_frame_growth,
        audio_route_sequence,
        media.nonblack_pixels,
        media.pcm_rms,
        first_video.timestamp_rate_ppm,
        first_audio.timestamp_rate_ppm,
        video.timestamp_rate_ppm,
        video.arrival_skew_ns.value(),
        video.observation_window_nanoseconds.value(),
        video.rate_standard_error_ppm,
        video.timestamp_residual_rms.value(),
        audio.timestamp_rate_ppm,
        audio.arrival_skew_ns.value(),
        audio.observation_window_nanoseconds.value(),
        audio.rate_standard_error_ppm,
        audio.timestamp_residual_rms.value(),
        samples.max_video_queue_ns,
        samples.max_audio_queue_ns,
        samples.rss_growth_kib(),
        segments.len()
    );
    assert!(audio_route_sequence > 0);
    assert!(media.nonblack_pixels > 500);
    assert!(media.pcm_rms > 50.0);
    assert!((800..=2_200).contains(&first_video.timestamp_rate_ppm));
    assert!((800..=2_200).contains(&first_audio.timestamp_rate_ppm));
    assert!((-11_000..=-3_000).contains(&video.timestamp_rate_ppm));
    assert!((-11_000..=-3_000).contains(&audio.timestamp_rate_ppm));
    assert!(video.discontinuities.value() >= 1);
    assert!(audio.discontinuities.value() >= 1);
    assert!(!disconnected.source.connected);
    assert_eq!(final_snapshot.source.continuity_generation.value(), 2);
    assert!(samples.program_frame_growth >= started.elapsed().as_secs().saturating_mul(24));
    assert_eq!(first_video.samples.value(), 4_096);
    assert_eq!(first_audio.samples.value(), 4_096);
    assert!(samples.max_video_queue_ns <= 4_000_000_000);
    assert!(samples.max_audio_queue_ns <= 4_000_000_000);
    assert!(samples.rss_growth_kib() <= 524_288);
    assert!(segments.len() >= 8);
    assert!(
        segments
            .iter()
            .all(|segment| segment.bytes > 0 && segment.decoded)
    );
    Ok(())
}

#[derive(Default)]
struct RunSamples {
    warm_rss_kib: Option<u64>,
    max_rss_kib: u64,
    first_program_frames: Option<u64>,
    program_frame_growth: u64,
    max_video_queue_ns: u64,
    max_audio_queue_ns: u64,
    sequence: u32,
}

impl RunSamples {
    fn observe(&mut self, sample: &RuntimeSample, rss: u64) {
        if self.warm_rss_kib.is_none() && self.sequence >= 5 {
            self.warm_rss_kib = Some(rss);
        }
        self.max_rss_kib = self.max_rss_kib.max(rss);
        self.max_video_queue_ns = self
            .max_video_queue_ns
            .max(sample.source.video_buffered_ns.value());
        self.max_audio_queue_ns = self
            .max_audio_queue_ns
            .max(sample.source.audio_buffered_ns.value());
        let frames = sample.program.video_frames.value();
        let first = *self.first_program_frames.get_or_insert(frames);
        self.program_frame_growth = frames.saturating_sub(first);
    }

    fn rss_growth_kib(&self) -> u64 {
        self.max_rss_kib
            .saturating_sub(self.warm_rss_kib.unwrap_or(self.max_rss_kib))
    }
}

fn run_phase(
    fixture: &Fixture,
    duration: Duration,
    samples: &mut RunSamples,
) -> anyhow::Result<RuntimeSample> {
    let deadline = Instant::now() + duration;
    let mut latest = None;
    while Instant::now() < deadline {
        samples.sequence = samples.sequence.saturating_add(1);
        match samples.sequence {
            10 => fixture.apply_timing("drift-timing-a", -250, 125)?,
            25 => fixture.apply_timing("drift-timing-b", 250, -100)?,
            40 => fixture.apply_timing("drift-timing-c", -100, 200)?,
            _ => {}
        }
        let snapshot = fixture.snapshot(&format!("drift-sample-{}", samples.sequence))?;
        samples.observe(&snapshot, rss_kib()?);
        latest = Some(snapshot);
        std::thread::sleep(Duration::from_secs(1));
    }
    latest.ok_or_else(|| anyhow::anyhow!("sustained phase produced no samples"))
}

fn run_reconnect_phase(
    fixture: &Fixture,
    samples: &mut RunSamples,
) -> anyhow::Result<RuntimeSample> {
    let deadline = Instant::now() + Duration::from_secs(18);
    let mut latest = None;
    while Instant::now() < deadline {
        samples.sequence = samples.sequence.saturating_add(1);
        let snapshot = fixture.snapshot(&format!("drift-reconnect-{}", samples.sequence))?;
        samples.observe(&snapshot, rss_kib()?);
        let ready = snapshot.source.video_drift.as_ref().is_some_and(|value| {
            value.discontinuities.value() >= 1
                && value.observation_window_nanoseconds.value() >= 8_000_000_000
        }) && snapshot.source.audio_drift.as_ref().is_some_and(|value| {
            value.discontinuities.value() >= 1
                && value.observation_window_nanoseconds.value() >= 8_000_000_000
        });
        latest = Some(snapshot);
        if ready {
            return latest.ok_or_else(|| anyhow::anyhow!("missing reconnect snapshot"));
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    Err(anyhow::anyhow!(
        "drift reports did not reset after reconnect: {latest:?}"
    ))
}

fn report(snapshot: &SourceSnapshot, video: bool) -> anyhow::Result<&TrackDriftReport> {
    if video {
        snapshot.video_drift.as_ref()
    } else {
        snapshot.audio_drift.as_ref()
    }
    .ok_or_else(|| anyhow::anyhow!("drift report missing"))
}
