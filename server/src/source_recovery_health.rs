use crate::{
    source_recovery::{HealthObservation, SourceContext},
    source_recovery_state::{cancel, parse},
    source_recovery_types::{ReceiverState, RecoveryRecord, RetryRecord, RetryResult},
};

const DEBOUNCE_NS: u64 = 3_000_000_000;

pub fn apply_observation(
    record: &mut RecoveryRecord,
    source: &SourceContext,
    observation: HealthObservation,
    now: u64,
) {
    if !source.enabled {
        record.receiver.state = ReceiverState::SourceDisabled;
        cancel(record);
        return;
    }
    let generation_changed =
        record.receiver.continuity_generation.as_ref() != Some(&observation.generation);
    let video = increased(
        record.receiver.video_frames.as_deref(),
        &observation.video_frames,
    );
    let audio = increased(
        record.receiver.audio_buffers.as_deref(),
        &observation.audio_buffers,
    );
    let pts = match (
        &record.receiver.last_video_pts_ns,
        &observation.last_video_pts_ns,
    ) {
        (Some(previous), Some(current)) => increased(Some(previous), current),
        _ => false,
    };
    record.receiver.continuity_generation = Some(observation.generation);
    record.receiver.video_frames = Some(observation.video_frames);
    record.receiver.audio_buffers = Some(observation.audio_buffers);
    record.receiver.last_video_pts_ns = observation.last_video_pts_ns;
    if generation_changed {
        record.retry.healthy_observations = 0;
        record.receiver.generation_started_ns = Some(now.to_string());
        record.receiver.last_progress_ns = None;
        record.receiver.state = ReceiverState::Warming;
        return;
    }
    if video && audio && pts {
        record.receiver.last_progress_ns = Some(now.to_string());
        record.receiver.state = ReceiverState::Healthy;
        let generation_advanced = record
            .retry
            .baseline_generation
            .as_deref()
            .and_then(parse)
            .zip(
                record
                    .receiver
                    .continuity_generation
                    .as_deref()
                    .and_then(parse),
            )
            .is_some_and(|(baseline, current)| current > baseline);
        if record.retry.last_result == RetryResult::Started && generation_advanced {
            record.retry.healthy_observations = record.retry.healthy_observations.saturating_add(1);
            if record.retry.healthy_observations >= 2 {
                record.retry = RetryRecord {
                    last_result: RetryResult::Recovered,
                    request_ids: std::mem::take(&mut record.retry.request_ids),
                    ..RetryRecord::default()
                };
            }
        }
        return;
    }
    record.retry.healthy_observations = 0;
    let start = record
        .receiver
        .last_progress_ns
        .as_deref()
        .and_then(parse)
        .or_else(|| {
            record
                .receiver
                .generation_started_ns
                .as_deref()
                .and_then(parse)
        })
        .unwrap_or(now)
        .max(
            record
                .retry
                .action_grace_started_ns
                .as_deref()
                .and_then(parse)
                .unwrap_or(0),
        );
    record.receiver.state = if now.saturating_sub(start) >= DEBOUNCE_NS {
        ReceiverState::Stalled
    } else {
        ReceiverState::Warming
    };
}

fn increased(previous: Option<&str>, current: &str) -> bool {
    previous
        .and_then(parse)
        .zip(parse(current))
        .is_some_and(|(previous, current)| current > previous)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_recovery_types::{RecoveryRecord, RetryResult};

    fn source() -> SourceContext {
        SourceContext {
            id: "source".into(),
            enabled: true,
            srt: true,
        }
    }
    fn observation(generation: &str, video: &str, audio: &str, pts: &str) -> HealthObservation {
        HealthObservation {
            generation: generation.into(),
            video_frames: video.into(),
            audio_buffers: audio.into(),
            last_video_pts_ns: Some(pts.into()),
        }
    }

    #[test]
    fn health_requires_both_tracks_and_pts_and_generation_grace() {
        let mut record = RecoveryRecord::new("source");
        apply_observation(&mut record, &source(), observation("1", "0", "0", "0"), 0);
        assert_eq!(record.receiver.state, ReceiverState::Warming);
        apply_observation(&mut record, &source(), observation("1", "1", "0", "1"), 1);
        assert_eq!(record.receiver.state, ReceiverState::Warming);
        apply_observation(
            &mut record,
            &source(),
            observation("1", "2", "1", "1"),
            3_000_000_001,
        );
        assert_eq!(record.receiver.state, ReceiverState::Stalled);
        apply_observation(
            &mut record,
            &source(),
            observation("2", "0", "0", "0"),
            3_000_000_002,
        );
        assert_eq!(record.receiver.state, ReceiverState::Warming);
    }

    #[test]
    fn two_strict_healthy_observations_recover_started_attempt() {
        let mut record = RecoveryRecord::new("source");
        record.retry.last_result = RetryResult::Started;
        record.retry.baseline_generation = Some("0".into());
        apply_observation(&mut record, &source(), observation("1", "1", "1", "1"), 0);
        apply_observation(&mut record, &source(), observation("1", "2", "2", "2"), 1);
        assert_eq!(record.retry.healthy_observations, 1);
        apply_observation(&mut record, &source(), observation("1", "3", "3", "3"), 2);
        assert_eq!(record.retry.last_result, RetryResult::Recovered);
    }
}
