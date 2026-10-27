use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Instant,
};

use gst::prelude::*;
use gstreamer as gst;
use num_traits::ToPrimitive;

use crate::protocol::{FrameCount, Nanoseconds, SignedNanoseconds, TrackDriftReport};

const MAX_SAMPLES: usize = 4_096;
const MIN_SAMPLES: usize = 20;
const MIN_WINDOW_NS: u64 = 1_000_000_000;
const DISCONTINUITY_NS: u64 = 1_000_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DriftEstimate {
    pub samples: u64,
    pub timestamp_rate_ppm: i64,
    pub rate_standard_error_ppm: u64,
    pub arrival_skew_ns: i64,
    pub observation_window_nanoseconds: u64,
    pub timestamp_residual_rms_ns: u64,
    pub discontinuities: u64,
}

impl From<DriftEstimate> for TrackDriftReport {
    fn from(value: DriftEstimate) -> Self {
        Self {
            samples: FrameCount::new(value.samples),
            timestamp_rate_ppm: value.timestamp_rate_ppm,
            rate_standard_error_ppm: value.rate_standard_error_ppm,
            arrival_skew_ns: SignedNanoseconds::new(value.arrival_skew_ns),
            observation_window_nanoseconds: Nanoseconds::new(value.observation_window_nanoseconds),
            timestamp_residual_rms: Nanoseconds::new(value.timestamp_residual_rms_ns),
            discontinuities: FrameCount::new(value.discontinuities),
        }
    }
}

#[derive(Default)]
struct DriftState {
    points: VecDeque<(u64, u64)>,
    discontinuities: u64,
}

pub struct DriftObserver {
    origin: Instant,
    state: Mutex<DriftState>,
}

impl Default for DriftObserver {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
            state: Mutex::new(DriftState::default()),
        }
    }
}

impl DriftObserver {
    pub fn observe(&self, pts_ns: u64, arrival_ns: u64) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if discontinuous(&state.points, pts_ns, arrival_ns) {
            state.points.clear();
            state.discontinuities = state.discontinuities.saturating_add(1);
        }
        if state.points.len() == MAX_SAMPLES {
            state.points.pop_front();
        }
        state.points.push_back((arrival_ns, pts_ns));
    }

    pub fn estimate(&self) -> Option<DriftEstimate> {
        let state = self.state.lock().ok()?;
        estimate(&state)
    }

    fn observe_now(&self, pts_ns: u64) {
        let arrival_ns = self
            .origin
            .elapsed()
            .as_nanos()
            .try_into()
            .unwrap_or(u64::MAX);
        self.observe(pts_ns, arrival_ns);
    }
}

pub fn add_drift_probe(
    bin: &gst::Bin,
    element_name: &str,
    observer: Arc<DriftObserver>,
) -> Result<(), String> {
    let element = bin
        .by_name(element_name)
        .ok_or_else(|| format!("source has no '{element_name}' element"))?;
    let pad = element
        .static_pad("sink")
        .ok_or_else(|| format!("'{element_name}' has no sink pad"))?;
    pad.add_probe(gst::PadProbeType::BUFFER, move |_, info| {
        if let Some(pts) = info.buffer().and_then(|buffer| buffer.pts()) {
            observer.observe_now(pts.nseconds());
        }
        gst::PadProbeReturn::Ok
    })
    .ok_or_else(|| format!("failed to observe drift at '{element_name}'"))?;
    Ok(())
}

pub fn install_drift_probes(
    bin: &gst::Bin,
    video_element: &str,
    audio_element: &str,
) -> Result<(Arc<DriftObserver>, Arc<DriftObserver>), String> {
    let video = Arc::new(DriftObserver::default());
    let audio = Arc::new(DriftObserver::default());
    add_drift_probe(bin, video_element, Arc::clone(&video))?;
    add_drift_probe(bin, audio_element, Arc::clone(&audio))?;
    Ok((video, audio))
}

fn discontinuous(points: &VecDeque<(u64, u64)>, pts_ns: u64, arrival_ns: u64) -> bool {
    let Some((last_arrival, last_pts)) = points.back().copied() else {
        return false;
    };
    if pts_ns <= last_pts || arrival_ns <= last_arrival {
        return true;
    }
    let arrival_step = arrival_ns - last_arrival;
    let pts_step = pts_ns - last_pts;
    arrival_step > DISCONTINUITY_NS || pts_step.abs_diff(arrival_step) > DISCONTINUITY_NS
}

fn estimate(state: &DriftState) -> Option<DriftEstimate> {
    if state.points.len() < MIN_SAMPLES {
        return None;
    }
    let (first_arrival, first_pts) = *state.points.front()?;
    let (last_arrival, last_pts) = *state.points.back()?;
    let window = last_arrival.checked_sub(first_arrival)?;
    if window < MIN_WINDOW_NS {
        return None;
    }
    let values: Vec<_> = state
        .points
        .iter()
        .map(|(arrival, pts)| {
            Some((
                (*arrival - first_arrival).to_f64()? / 1_000_000_000.0,
                (*pts - first_pts).to_f64()? / 1_000_000_000.0,
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    let count = values.len().to_f64()?;
    let mean_x = values.iter().map(|(x, _)| x).sum::<f64>() / count;
    let mean_y = values.iter().map(|(_, y)| y).sum::<f64>() / count;
    let sxx = values
        .iter()
        .map(|(x, _)| (x - mean_x).powi(2))
        .sum::<f64>();
    if sxx <= f64::EPSILON {
        return None;
    }
    let slope = values
        .iter()
        .map(|(x, y)| (x - mean_x) * (y - mean_y))
        .sum::<f64>()
        / sxx;
    let intercept = mean_y - slope * mean_x;
    let residual = values
        .iter()
        .map(|(x, y)| (y - intercept - slope * x).powi(2))
        .sum::<f64>();
    let variance = residual / (count - 2.0).max(1.0);
    let uncertainty_seconds = variance.sqrt();
    let rate_uncertainty = (variance / sxx).sqrt() * 1_000_000.0;
    Some(DriftEstimate {
        samples: values.len().try_into().ok()?,
        timestamp_rate_ppm: ((slope - 1.0) * 1_000_000.0).round().to_i64()?,
        rate_standard_error_ppm: rate_uncertainty.ceil().max(0.0).to_u64()?,
        arrival_skew_ns: i64::try_from(last_pts - first_pts).ok()? - i64::try_from(window).ok()?,
        observation_window_nanoseconds: window,
        timestamp_residual_rms_ns: (uncertainty_seconds * 1_000_000_000.0).ceil().to_u64()?,
        discontinuities: state.discontinuities,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_history_remains_bounded_after_long_runs() -> anyhow::Result<()> {
        let observer = DriftObserver::default();
        for index in 0..20_000_u64 {
            observer.observe(index * 33_333_333, index * 33_333_333);
        }
        let report = observer
            .estimate()
            .ok_or_else(|| anyhow::anyhow!("bounded report"))?;
        assert_eq!(report.samples, 4_096);
        assert_eq!(report.timestamp_rate_ppm, 0);
        Ok(())
    }

    #[test]
    fn estimates_positive_and_negative_timestamp_rate() -> anyhow::Result<()> {
        let positive = estimate_fixture(1_001_500)?;
        let negative = estimate_fixture(998_800)?;
        assert!((1_450..=1_550).contains(&positive.timestamp_rate_ppm));
        assert!((-1_250..=-1_150).contains(&negative.timestamp_rate_ppm));
        assert!(positive.timestamp_residual_rms_ns <= 1);
        assert!(negative.timestamp_residual_rms_ns <= 1);
        Ok(())
    }

    #[test]
    fn timestamp_reset_starts_a_new_window_and_reports_discontinuity() -> anyhow::Result<()> {
        let observer = DriftObserver::default();
        for index in 0..60_u64 {
            observer.observe(index * 33_333_333, index * 33_333_333);
        }
        observer.observe(0, 2_000_000_000);
        for index in 1..60_u64 {
            observer.observe(index * 33_333_333, (60 + index) * 33_333_333);
        }
        let report = observer
            .estimate()
            .ok_or_else(|| anyhow::anyhow!("new drift window missing"))?;
        assert_eq!(report.discontinuities, 1);
        assert!(report.observation_window_nanoseconds < 2_100_000_000);
        Ok(())
    }

    fn estimate_fixture(rate_ppm: u64) -> anyhow::Result<DriftEstimate> {
        let observer = DriftObserver::default();
        for index in 0..300_u64 {
            let arrival = index * 33_333_333;
            let pts = arrival.saturating_mul(rate_ppm) / 1_000_000;
            observer.observe(pts, arrival);
        }
        observer
            .estimate()
            .ok_or_else(|| anyhow::anyhow!("drift estimate missing"))
    }
}
