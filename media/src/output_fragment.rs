use std::{
    fs::{self, File},
    io::Read,
    path::Path,
    sync::{Arc, Mutex},
};

use gst::prelude::*;
use gstreamer as gst;
use sha2::{Digest, Sha256};

use crate::{
    output::{
        OutputError, RecordingMapping, RecordingProvenanceGap, RecordingProvenanceInterval,
        RecordingRecovery, RecordingSegment,
    },
    output_fragment_provenance::{clip_gaps, clip_intervals},
    recording_clock::RecordingClock,
};

#[derive(Clone, Copy)]
pub struct FragmentContext<'a> {
    pub session_id: &'a str,
    pub wall_start_ns: Option<u64>,
    pub wall_end_ns: Option<u64>,
    pub engine_run_id: Option<&'a str>,
    pub clock: Option<&'a RecordingClock>,
    pub recovery: RecordingRecovery,
    pub provenance: &'a [RecordingProvenanceInterval],
    pub provenance_gaps: &'a [RecordingProvenanceGap],
}

#[derive(Clone, Copy, Default)]
struct PacketBounds {
    first: Option<u64>,
    end: Option<u64>,
    uncertainty: u64,
}

pub fn inspect(
    path: &Path,
    context: FragmentContext<'_>,
    gaps: Vec<String>,
) -> Result<RecordingSegment, OutputError> {
    let bytes = fs::metadata(path)
        .map_err(|_| OutputError::RecordingFinalization)?
        .len();
    let sha256 = checksum(path)?;
    let decoded = decode(path).is_ok();
    let bounds = packet_bounds(path).ok();
    let provenance = bounds.map_or_else(Vec::new, |value| {
        value
            .first
            .zip(value.end)
            .map_or_else(Vec::new, |(start, end)| {
                clip_intervals(start, end, context.provenance)
            })
    });
    let provenance_gaps = bounds.map_or_else(Vec::new, |value| {
        value
            .first
            .zip(value.end)
            .map_or_else(Vec::new, |(start, end)| {
                clip_gaps(start, end, context.provenance_gaps)
            })
    });
    let calibration = context.clock.and_then(RecordingClock::calibration);
    let calibrated_start = mapped_bound(calibration, bounds.and_then(|value| value.first));
    let calibrated_end = mapped_bound(calibration, bounds.and_then(|value| value.end));
    let provenance_start = provenance.iter().map(|value| value.media_start_ns).min();
    let provenance_end = provenance.iter().map(|value| value.media_end_ns).max();
    let media_start_ns = calibrated_start.zip(provenance_start).map_or_else(
        || calibrated_start.or(provenance_start),
        |(left, right)| Some(left.min(right)),
    );
    let media_end_ns = calibrated_end.zip(provenance_end).map_or_else(
        || calibrated_end.or(provenance_end),
        |(left, right)| Some(left.max(right)),
    );
    let media_uncertainty_ns = calibration
        .and_then(|(_, uncertainty)| {
            bounds.map(|value| uncertainty.saturating_add(value.uncertainty))
        })
        .or_else(|| {
            bounds
                .map(|value| value.uncertainty)
                .filter(|_| !provenance.is_empty())
        });
    let mapping = if media_start_ns.is_some() && media_end_ns.is_some() {
        RecordingMapping::Mapped
    } else {
        RecordingMapping::Unmapped
    };
    let recovery = if decoded {
        context.recovery
    } else {
        RecordingRecovery::Partial
    };
    Ok(RecordingSegment {
        path: path.to_path_buf(),
        start_ns: context.wall_start_ns,
        end_ns: context.wall_end_ns,
        bytes,
        sha256,
        decoded,
        session_id: context.session_id.to_owned(),
        recovery,
        mapping,
        gaps,
        engine_run_id: context
            .clock
            .and_then(|clock| clock.engine_run_id.clone())
            .or_else(|| provenance.first().map(|value| value.engine_run_id.clone()))
            .or_else(|| context.engine_run_id.map(str::to_owned)),
        media_start_ns,
        media_end_ns,
        media_uncertainty_ns,
        provenance,
        provenance_gaps,
    })
}

fn mapped_bound(calibration: Option<(i128, u64)>, bound: Option<u64>) -> Option<u64> {
    calibration
        .zip(bound)
        .and_then(|((origin, _), value)| u64::try_from(origin + i128::from(value)).ok())
}

fn checksum(path: &Path) -> Result<String, OutputError> {
    let mut file = File::open(path).map_err(|_| OutputError::RecordingFinalization)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| OutputError::RecordingFinalization)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn decode(path: &Path) -> Result<(), OutputError> {
    let location = quote_path(path)?;
    let pipeline = gst::parse::launch(&format!(
        "filesrc location={location} ! matroskademux name=demux \
         demux. ! queue ! decodebin ! fakesink sync=false \
         demux. ! queue ! decodebin ! fakesink sync=false"
    ))
    .map_err(|_| OutputError::RecordingFinalization)?
    .downcast::<gst::Pipeline>()
    .map_err(|_| OutputError::RecordingFinalization)?;
    run_to_eos(&pipeline)
}

fn packet_bounds(path: &Path) -> Result<PacketBounds, OutputError> {
    let location = quote_path(path)?;
    let pipeline = gst::parse::launch(&format!(
        "filesrc location={location} ! matroskademux name=demux \
         demux. ! queue ! h264parse ! identity name=video_probe ! fakesink sync=false \
         demux. ! queue ! aacparse ! identity name=audio_probe ! fakesink sync=false"
    ))
    .map_err(|_| OutputError::RecordingFinalization)?
    .downcast::<gst::Pipeline>()
    .map_err(|_| OutputError::RecordingFinalization)?;
    let bounds = Arc::new(Mutex::new(PacketBounds::default()));
    let _video = timing_probe(&pipeline, "video_probe", &bounds)?;
    let _audio = timing_probe(&pipeline, "audio_probe", &bounds)?;
    run_to_eos(&pipeline)?;
    bounds
        .lock()
        .map(|value| *value)
        .map_err(|_| OutputError::RecordingFinalization)
}

fn timing_probe(
    pipeline: &gst::Pipeline,
    name: &str,
    bounds: &Arc<Mutex<PacketBounds>>,
) -> Result<gst::PadProbeId, OutputError> {
    let pad = pipeline
        .by_name(name)
        .and_then(|identity| identity.static_pad("src"))
        .ok_or(OutputError::RecordingFinalization)?;
    let observed = Arc::clone(bounds);
    pad.add_probe(gst::PadProbeType::BUFFER, move |_, info| {
        if let Some(buffer) = info.buffer()
            && let Some(pts) = buffer.pts().or_else(|| buffer.dts())
            && let Ok(mut value) = observed.lock()
        {
            let start = pts.nseconds();
            let duration = buffer.duration().map_or(0, gst::ClockTime::nseconds);
            value.first = Some(value.first.map_or(start, |first| first.min(start)));
            let end = start.saturating_add(duration);
            value.end = Some(value.end.map_or(end, |current| current.max(end)));
            value.uncertainty = value.uncertainty.max(duration);
        }
        gst::PadProbeReturn::Ok
    })
    .ok_or(OutputError::RecordingFinalization)
}

fn run_to_eos(pipeline: &gst::Pipeline) -> Result<(), OutputError> {
    let bus = pipeline.bus().ok_or(OutputError::RecordingFinalization)?;
    pipeline
        .set_state(gst::State::Playing)
        .map_err(|_| OutputError::RecordingFinalization)?;
    let message = bus.timed_pop_filtered(
        gst::ClockTime::from_seconds(30),
        &[gst::MessageType::Eos, gst::MessageType::Error],
    );
    let reached_eos = message
        .as_ref()
        .is_some_and(|message| matches!(message.view(), gst::MessageView::Eos(_)));
    pipeline
        .set_state(gst::State::Null)
        .map_err(|_| OutputError::RecordingFinalization)?;
    reached_eos
        .then_some(())
        .ok_or(OutputError::RecordingFinalization)
}

fn quote_path(path: &Path) -> Result<String, OutputError> {
    let value = path
        .to_str()
        .ok_or(OutputError::RecordingFinalization)?
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    Ok(format!("\"{value}\""))
}
