use std::sync::{
    Arc,
    atomic::{AtomicI64, AtomicU64, Ordering},
};

use gst::prelude::*;
use gstreamer as gst;

#[derive(Default)]
pub struct TrackProgress {
    buffers: AtomicU64,
    last_pts_ns: AtomicU64,
    last_activity_ns: AtomicU64,
}

#[derive(Clone, Copy)]
pub struct ProgressNames<'a> {
    pub video_playout: &'a str,
    pub audio_playout: &'a str,
    pub video_ingest: &'a str,
    pub audio_ingest: &'a str,
}

pub struct ProgressObservers {
    pub video_playout: Arc<TrackProgress>,
    pub audio_playout: Arc<TrackProgress>,
    pub video_ingest: Arc<TrackProgress>,
    pub audio_ingest: Arc<TrackProgress>,
}

pub fn install_progress_probes(
    bin: &gst::Bin,
    names: ProgressNames<'_>,
) -> Result<ProgressObservers, String> {
    let observers = ProgressObservers {
        video_playout: Arc::new(TrackProgress::default()),
        audio_playout: Arc::new(TrackProgress::default()),
        video_ingest: Arc::new(TrackProgress::default()),
        audio_ingest: Arc::new(TrackProgress::default()),
    };
    add_progress_probe(
        bin,
        names.video_playout,
        Arc::clone(&observers.video_playout),
    )?;
    add_progress_probe(
        bin,
        names.audio_playout,
        Arc::clone(&observers.audio_playout),
    )?;
    add_progress_probe(bin, names.video_ingest, Arc::clone(&observers.video_ingest))?;
    add_progress_probe(bin, names.audio_ingest, Arc::clone(&observers.audio_ingest))?;
    Ok(observers)
}

#[derive(Default)]
pub struct TimingControl(AtomicI64);

impl TimingControl {
    pub fn set_offset(&self, value: i64) {
        self.0.store(value, Ordering::Relaxed);
    }

    pub fn offset(&self) -> i64 {
        self.0.load(Ordering::Relaxed)
    }
}

pub fn add_timing_probe(
    bin: &gst::Bin,
    element_name: &str,
    control: Arc<TimingControl>,
) -> Result<(), String> {
    let element = bin
        .by_name(element_name)
        .ok_or_else(|| format!("source has no '{element_name}' element"))?;
    let pad = element
        .static_pad("src")
        .ok_or_else(|| format!("'{element_name}' has no src pad"))?;
    pad.add_probe(gst::PadProbeType::BUFFER, move |_, info| {
        let Some(buffer) = info.buffer_mut() else {
            return gst::PadProbeReturn::Ok;
        };
        let offset = control.0.load(Ordering::Relaxed);
        let buffer = buffer.make_mut();
        if let Some(pts) = buffer.pts() {
            buffer.set_pts(Some(gst::ClockTime::from_nseconds(
                pts.nseconds().saturating_add_signed(offset),
            )));
        }
        if let Some(dts) = buffer.dts() {
            buffer.set_dts(Some(gst::ClockTime::from_nseconds(
                dts.nseconds().saturating_add_signed(offset),
            )));
        }
        gst::PadProbeReturn::Ok
    })
    .ok_or_else(|| format!("failed to time '{element_name}'"))?;
    Ok(())
}

impl TrackProgress {
    pub fn buffers(&self) -> u64 {
        self.buffers.load(Ordering::Relaxed)
    }

    pub fn last_activity_ns(&self) -> u64 {
        self.last_activity_ns.load(Ordering::Relaxed)
    }

    pub fn last_pts_ns(&self) -> Option<u64> {
        let value = self.last_pts_ns.load(Ordering::Relaxed);
        (value != 0).then_some(value)
    }
}

pub fn add_progress_probe(
    bin: &gst::Bin,
    element_name: &str,
    progress: Arc<TrackProgress>,
) -> Result<(), String> {
    let element = bin
        .by_name(element_name)
        .ok_or_else(|| format!("source has no '{element_name}' element"))?;
    let pad = element
        .static_pad("src")
        .ok_or_else(|| format!("'{element_name}' has no src pad"))?;
    pad.add_probe(gst::PadProbeType::BUFFER, move |_, info| {
        progress.buffers.fetch_add(1, Ordering::Relaxed);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| {
                duration.as_nanos().try_into().unwrap_or(u64::MAX)
            });
        progress.last_activity_ns.store(now, Ordering::Relaxed);
        if let Some(pts) = info.buffer().and_then(|buffer| buffer.pts()) {
            progress
                .last_pts_ns
                .store(pts.nseconds(), Ordering::Relaxed);
        }
        gst::PadProbeReturn::Ok
    })
    .ok_or_else(|| format!("failed to observe '{element_name}'"))?;
    Ok(())
}

pub fn queue_level(pipeline: &gst::Pipeline, element_name: &str) -> u64 {
    pipeline
        .by_name(element_name)
        .map_or(0, |queue| queue.property::<u64>("current-level-time"))
}
