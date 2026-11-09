use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use gst::prelude::*;
use gstreamer as gst;

use crate::{
    engine_source_epoch_state::{EpochControl, EpochState},
    engine_source_graph::SourceNames,
    protocol::{EngineRunId, SourceId, SourceSpec, TrackId},
};

pub struct EpochReference<'a> {
    pub run: &'a EngineRunId,
    pub source: &'a SourceId,
    pub track: &'a TrackId,
    pub kind: &'static str,
    pub initial_generation: u64,
}

pub struct SourceEpochSetup<'a> {
    pub bin: &'a gst::Bin,
    pub names: &'a SourceNames,
    pub pipeline: &'a gst::Pipeline,
    pub run: &'a EngineRunId,
    pub spec: &'a SourceSpec,
    pub generation: u64,
}

pub struct SourceEpoch {
    fence: Arc<AtomicBool>,
    pad: gst::glib::WeakRef<gst::Pad>,
    probe: Mutex<Option<gst::PadProbeId>>,
}

pub struct SourceEpochPair {
    video: SourceEpoch,
    audio: SourceEpoch,
    control: Arc<EpochControl>,
}

impl SourceEpochPair {
    pub fn authorized_generation(&self) -> u64 {
        self.control.authorized_generation()
    }

    pub fn begin_flush(&self) {
        self.control.begin_flush();
    }

    pub fn complete_flush(&self, generation: u64) {
        self.control.authorize(generation);
        self.video.fence();
        self.audio.fence();
        self.control.resume();
    }

    pub fn disconnect(&self) {
        self.video.disconnect();
        self.audio.disconnect();
    }
}

impl SourceEpoch {
    pub fn fence(&self) {
        self.fence.store(true, Ordering::Release);
    }

    fn disconnect(&self) {
        let Some(pad) = self.pad.upgrade() else {
            return;
        };
        let Ok(mut probe) = self.probe.lock() else {
            return;
        };
        if let Some(probe) = probe.take() {
            pad.remove_probe(probe);
        }
    }
}

pub fn install(
    pad: &gst::Pad,
    pipeline: &gst::Pipeline,
    reference: &EpochReference<'_>,
    control: &Arc<EpochControl>,
) -> Result<SourceEpoch, String> {
    let run = reference.run.as_str().to_owned();
    let source = reference.source.as_str().to_owned();
    let track = reference.track.as_str().to_owned();
    let kind = reference.kind;
    let initial_generation = reference.initial_generation;
    let pipeline = pipeline.downgrade();
    let state = Arc::new(Mutex::new(EpochState::new(initial_generation)));
    let fence = Arc::new(AtomicBool::new(false));
    let probe_fence = Arc::clone(&fence);
    let probe_control = Arc::clone(control);
    let probe = pad
        .add_probe(
            gst::PadProbeType::BUFFER | gst::PadProbeType::EVENT_DOWNSTREAM,
            move |_, info| {
                if info
                    .event()
                    .is_some_and(|event| matches!(event.view(), gst::EventView::Segment(_)))
                {
                    if let Ok(mut value) = state.lock() {
                        value.segment_pending = true;
                    }
                    // Buffers below are rewritten into the engine clock domain.
                    // Keep downstream running-time conversion in that same domain.
                    let segment = gst::FormattedSegment::<gst::ClockTime>::new();
                    info.data = Some(gst::PadProbeData::Event(gst::event::Segment::new(&segment)));
                    return gst::PadProbeReturn::Ok;
                }
                let Some(buffer) = info.buffer_mut() else {
                    return gst::PadProbeReturn::Ok;
                };
                let Some(authorized_generation) = probe_control.generation_for_buffer() else {
                    return gst::PadProbeReturn::Drop;
                };
                let raw = buffer.pts().map_or(0, gst::ClockTime::nseconds);
                let duration = buffer.duration().map_or(0, gst::ClockTime::nseconds);
                let Some(pipeline) = pipeline.upgrade() else {
                    return gst::PadProbeReturn::Drop;
                };
                let running = pipeline
                    .current_running_time()
                    .map_or(0, gst::ClockTime::nseconds);
                let discontinuity = buffer.flags().contains(gst::BufferFlags::DISCONT);
                let fenced = probe_fence.swap(false, Ordering::AcqRel);
                let Ok(mut epoch) = state.lock() else {
                    return gst::PadProbeReturn::Ok;
                };
                let (normalized, offset, latched_generation) = epoch.normalize(
                    raw,
                    duration,
                    running,
                    discontinuity || fenced,
                    authorized_generation,
                );
                let buffer = buffer.make_mut();
                buffer.set_pts(Some(gst::ClockTime::from_nseconds(normalized)));
                if let Some(dts) = buffer
                    .dts()
                    .and_then(|value| apply_offset(value.nseconds(), offset))
                {
                    buffer.set_dts(Some(gst::ClockTime::from_nseconds(dts)));
                }
                let caps = gst::Caps::builder("timestamp/x-live-production-engine")
                    .field("engine-run-id", run.as_str())
                    .field("source-id", source.as_str())
                    .field("track-id", track.as_str())
                    .field("track-kind", kind)
                    .field("continuity-generation", latched_generation)
                    .build();
                gst::ReferenceTimestampMeta::add(
                    buffer,
                    &caps,
                    gst::ClockTime::from_nseconds(normalized),
                    buffer.duration(),
                );
                gst::PadProbeReturn::Ok
            },
        )
        .ok_or_else(|| "cannot install serialized source epoch".to_owned())?;
    Ok(SourceEpoch {
        fence,
        pad: pad.downgrade(),
        probe: Mutex::new(Some(probe)),
    })
}

pub fn install_source_epochs(setup: &SourceEpochSetup<'_>) -> Result<SourceEpochPair, String> {
    let video = setup
        .bin
        .by_name(&setup.names.video_ingest)
        .and_then(|element| element.static_pad("src"))
        .ok_or_else(|| "source video has no reference timestamp pad".to_owned())?;
    let audio = setup
        .bin
        .by_name(&setup.names.audio_ingest)
        .and_then(|element| element.static_pad("src"))
        .ok_or_else(|| "source audio has no reference timestamp pad".to_owned())?;
    let control = Arc::new(EpochControl::new(setup.generation));
    let video_epoch = install(
        &video,
        setup.pipeline,
        &EpochReference {
            run: setup.run,
            source: &setup.spec.id,
            track: &setup.spec.video_track_id,
            kind: "video",
            initial_generation: setup.generation,
        },
        &control,
    )?;
    let audio_epoch = install(
        &audio,
        setup.pipeline,
        &EpochReference {
            run: setup.run,
            source: &setup.spec.id,
            track: &setup.spec.audio_track_id,
            kind: "audio",
            initial_generation: setup.generation,
        },
        &control,
    )?;
    Ok(SourceEpochPair {
        video: video_epoch,
        audio: audio_epoch,
        control,
    })
}

fn apply_offset(value: u64, offset: i128) -> Option<u64> {
    u64::try_from(i128::from(value) + offset).ok()
}
