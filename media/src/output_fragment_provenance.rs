use crate::output::{RecordingProvenanceGap, RecordingProvenanceInterval};

pub fn clip_intervals(
    fragment_start: u64,
    fragment_end: u64,
    intervals: &[RecordingProvenanceInterval],
) -> Vec<RecordingProvenanceInterval> {
    intervals
        .iter()
        .filter_map(|interval| {
            let local_start = interval.local_start_ns.max(fragment_start);
            let local_end = interval.local_end_ns.min(fragment_end);
            let media_start = interpolate(
                interval.local_start_ns,
                interval.local_end_ns,
                interval.media_start_ns,
                interval.media_end_ns,
                local_start,
            )?;
            let media_end = interpolate(
                interval.local_start_ns,
                interval.local_end_ns,
                interval.media_start_ns,
                interval.media_end_ns,
                local_end,
            )?;
            (local_start < local_end && media_start <= media_end).then(|| {
                RecordingProvenanceInterval {
                    engine_run_id: interval.engine_run_id.clone(),
                    source_id: interval.source_id.clone(),
                    track_id: interval.track_id.clone(),
                    track_kind: interval.track_kind,
                    continuity_generation: interval.continuity_generation,
                    local_start_ns: local_start,
                    local_end_ns: local_end,
                    media_start_ns: media_start,
                    media_end_ns: media_end,
                }
            })
        })
        .collect()
}

pub fn clip_gaps(
    fragment_start: u64,
    fragment_end: u64,
    gaps: &[RecordingProvenanceGap],
) -> Vec<RecordingProvenanceGap> {
    gaps.iter()
        .filter_map(|gap| {
            if gap.local_start_ns == gap.local_end_ns {
                return (fragment_start..=fragment_end)
                    .contains(&gap.local_start_ns)
                    .then(|| gap.clone())
                    .filter(|value| value.media_start_ns <= value.media_end_ns);
            }
            let local_start = gap.local_start_ns.max(fragment_start);
            let local_end = gap.local_end_ns.min(fragment_end);
            let media_start = interpolate(
                gap.local_start_ns,
                gap.local_end_ns,
                gap.media_start_ns,
                gap.media_end_ns,
                local_start,
            )?;
            let media_end = interpolate(
                gap.local_start_ns,
                gap.local_end_ns,
                gap.media_start_ns,
                gap.media_end_ns,
                local_end,
            )?;
            (local_start < local_end && media_start <= media_end).then(|| RecordingProvenanceGap {
                source_id: gap.source_id.clone(),
                track_id: gap.track_id.clone(),
                from_generation: gap.from_generation,
                to_generation: gap.to_generation,
                local_start_ns: local_start,
                local_end_ns: local_end,
                media_start_ns: media_start,
                media_end_ns: media_end,
            })
        })
        .collect()
}

fn interpolate(
    local_start: u64,
    local_end: u64,
    media_start: u64,
    media_end: u64,
    point: u64,
) -> Option<u64> {
    let local_span = local_end.checked_sub(local_start)?;
    let media_span = media_end.checked_sub(media_start)?;
    if local_span == 0 || !(local_start..=local_end).contains(&point) {
        return None;
    }
    let offset = point.checked_sub(local_start)?;
    let scaled = u128::from(media_span)
        .checked_mul(u128::from(offset))?
        .checked_div(u128::from(local_span))?;
    media_start.checked_add(u64::try_from(scaled).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::RecordingTrackKind;

    fn interval() -> RecordingProvenanceInterval {
        RecordingProvenanceInterval {
            engine_run_id: "run".into(),
            source_id: "source".into(),
            track_id: "video".into(),
            track_kind: RecordingTrackKind::Video,
            continuity_generation: 2,
            local_start_ns: 100,
            local_end_ns: 300,
            media_start_ns: 1_000,
            media_end_ns: 1_500,
        }
    }

    #[test]
    fn drifted_interval_and_gap_clip_proportionally() {
        let clipped = clip_intervals(150, 250, &[interval()]);
        assert_eq!(
            (clipped[0].media_start_ns, clipped[0].media_end_ns),
            (1_125, 1_375)
        );
        let gap = RecordingProvenanceGap {
            source_id: "source".into(),
            track_id: "video".into(),
            from_generation: 1,
            to_generation: 2,
            local_start_ns: 100,
            local_end_ns: 300,
            media_start_ns: 2_000,
            media_end_ns: 2_600,
        };
        let clipped_gap = clip_gaps(150, 250, &[gap]);
        assert_eq!(
            (clipped_gap[0].media_start_ns, clipped_gap[0].media_end_ns),
            (2_150, 2_450)
        );
        let point_gap = RecordingProvenanceGap {
            source_id: "source".into(),
            track_id: "video".into(),
            from_generation: 2,
            to_generation: 3,
            local_start_ns: 200,
            local_end_ns: 200,
            media_start_ns: 3_000,
            media_end_ns: 3_400,
        };
        assert_eq!(clip_gaps(150, 250, &[point_gap]).len(), 1);
    }

    #[test]
    fn invalid_ranges_are_not_clamped_into_fragments() {
        let mut invalid = interval();
        invalid.media_end_ns = invalid.media_start_ns - 1;
        assert!(clip_intervals(150, 250, &[invalid]).is_empty());
    }
}
