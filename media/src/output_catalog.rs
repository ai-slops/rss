use std::{
    fs::{self, File},
    io::Read,
    path::Path,
    sync::{Arc, Mutex},
};

use gst::prelude::*;
use gstreamer as gst;
use sha2::{Digest, Sha256};

use crate::output::{OutputError, RecordingSegment};
use crate::recording_clock::RecordingClock;

#[derive(Clone, Copy)]
pub struct CatalogWindow<'a> {
    pub wall_start_ns: u64,
    pub wall_end_ns: u64,
    pub clock: Option<&'a RecordingClock>,
}

#[derive(Clone, Copy, Default)]
struct PacketBounds {
    first: Option<u64>,
    end: Option<u64>,
    uncertainty: u64,
}

pub fn finalize(
    directory: &Path,
    window: CatalogWindow<'_>,
) -> Result<Vec<RecordingSegment>, OutputError> {
    let segments = inspect(directory, &window)?;
    write_manifest(directory, &segments)?;
    Ok(segments)
}

pub fn recover(
    directory: &Path,
    window: CatalogWindow<'_>,
) -> Result<Vec<RecordingSegment>, OutputError> {
    let segments = inspect(directory, &window)?;
    let _ = write_manifest(directory, &segments);
    Ok(segments)
}

fn inspect(
    directory: &Path,
    window: &CatalogWindow<'_>,
) -> Result<Vec<RecordingSegment>, OutputError> {
    let entries = fs::read_dir(directory).map_err(|_| OutputError::RecordingFinalization)?;
    let mut paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "mkv"))
        .collect::<Vec<_>>();
    paths.sort();
    let mut segments = Vec::with_capacity(paths.len());
    let calibration = window.clock.and_then(RecordingClock::calibration);
    for path in paths {
        let bytes = fs::metadata(&path)
            .map_err(|_| OutputError::RecordingFinalization)?
            .len();
        let sha256 = checksum(&path)?;
        let decoded = decode(&path).is_ok();
        let bounds = packet_bounds(&path).ok();
        let media_start_ns = calibration.and_then(|(origin, _)| {
            bounds
                .as_ref()
                .and_then(|value| value.first)
                .and_then(|value| u64::try_from(origin + i128::from(value)).ok())
        });
        let media_end_ns = calibration.and_then(|(origin, _)| {
            bounds
                .as_ref()
                .and_then(|value| value.end)
                .and_then(|value| u64::try_from(origin + i128::from(value)).ok())
        });
        segments.push(RecordingSegment {
            path,
            start_ns: window.wall_start_ns,
            end_ns: window.wall_end_ns,
            bytes,
            sha256,
            decoded,
            engine_run_id: window
                .clock
                .and_then(|mapping| mapping.engine_run_id.clone()),
            media_start_ns,
            media_end_ns,
            media_uncertainty_ns: calibration.and_then(|(_, uncertainty)| {
                bounds
                    .as_ref()
                    .map(|value| uncertainty.saturating_add(value.uncertainty))
            }),
        });
    }
    Ok(segments)
}

fn packet_bounds(path: &Path) -> Result<PacketBounds, OutputError> {
    let location = quote_path(path)?;
    let pipeline = gst::parse::launch(&format!(
        "filesrc location={location} ! matroskademux name=demux \
         demux. ! queue ! h264parse ! identity name=catalog_video_probe ! fakesink sync=false \
         demux. ! queue ! aacparse ! identity name=catalog_audio_probe ! fakesink sync=false"
    ))
    .map_err(|_| OutputError::RecordingFinalization)?
    .downcast::<gst::Pipeline>()
    .map_err(|_| OutputError::RecordingFinalization)?;
    let bounds = Arc::new(Mutex::new(PacketBounds::default()));
    let _video_probe = timing_probe(&pipeline, "catalog_video_probe", &bounds)?;
    let _audio_probe = timing_probe(&pipeline, "catalog_audio_probe", &bounds)?;
    run_to_eos(&pipeline)?;
    let value = *bounds
        .lock()
        .map_err(|_| OutputError::RecordingFinalization)?;
    Ok(value)
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
            let pts_ns = pts.nseconds();
            let duration_ns = buffer.duration().map_or(0, gst::ClockTime::nseconds);
            value.first = Some(value.first.map_or(pts_ns, |first| first.min(pts_ns)));
            value.end = Some(value.end.map_or_else(
                || pts_ns.saturating_add(duration_ns),
                |end| end.max(pts_ns.saturating_add(duration_ns)),
            ));
            value.uncertainty = value.uncertainty.max(duration_ns);
        }
        gst::PadProbeReturn::Ok
    })
    .ok_or(OutputError::RecordingFinalization)
}

fn write_manifest(directory: &Path, segments: &[RecordingSegment]) -> Result<(), OutputError> {
    let manifest =
        serde_json::to_vec_pretty(segments).map_err(|_| OutputError::RecordingFinalization)?;
    fs::write(directory.join("catalog.json"), manifest)
        .map_err(|_| OutputError::RecordingFinalization)
}

pub fn load(directory: &Path) -> Result<Vec<RecordingSegment>, OutputError> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Ok(Vec::new());
    };
    let mut recordings = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|_| OutputError::RecordingFinalization)?
            .path()
            .join("catalog.json");
        if !path.is_file() {
            continue;
        }
        let catalog = fs::read(&path).map_err(|_| OutputError::RecordingFinalization)?;
        let mut segments = serde_json::from_slice::<Vec<RecordingSegment>>(&catalog)
            .map_err(|_| OutputError::RecordingFinalization)?;
        recordings.append(&mut segments);
    }
    recordings.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(recordings)
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
    let description = format!(
        "filesrc location={location} ! matroskademux name=demux \
         demux. ! queue ! decodebin ! fakesink sync=false \
         demux. ! queue ! decodebin ! fakesink sync=false"
    );
    let pipeline = gst::parse::launch(&description)
        .map_err(|_| OutputError::RecordingFinalization)?
        .downcast::<gst::Pipeline>()
        .map_err(|_| OutputError::RecordingFinalization)?;
    run_to_eos(&pipeline)
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
