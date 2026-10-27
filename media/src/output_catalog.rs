use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

use gst::prelude::*;
use gstreamer as gst;
use sha2::{Digest, Sha256};

use crate::output::{OutputError, RecordingSegment};

pub fn finalize(
    directory: &Path,
    start_ns: u64,
    end_ns: u64,
) -> Result<Vec<RecordingSegment>, OutputError> {
    let segments = inspect(directory, start_ns, end_ns)?;
    write_manifest(directory, &segments)?;
    Ok(segments)
}

pub fn recover(
    directory: &Path,
    start_ns: u64,
    end_ns: u64,
) -> Result<Vec<RecordingSegment>, OutputError> {
    let segments = inspect(directory, start_ns, end_ns)?;
    let _ = write_manifest(directory, &segments);
    Ok(segments)
}

fn inspect(
    directory: &Path,
    start_ns: u64,
    end_ns: u64,
) -> Result<Vec<RecordingSegment>, OutputError> {
    let entries = fs::read_dir(directory).map_err(|_| OutputError::RecordingFinalization)?;
    let mut paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "mkv"))
        .collect::<Vec<_>>();
    paths.sort();
    let mut segments = Vec::with_capacity(paths.len());
    for path in paths {
        let bytes = fs::metadata(&path)
            .map_err(|_| OutputError::RecordingFinalization)?
            .len();
        let sha256 = checksum(&path)?;
        let decoded = decode(&path).is_ok();
        segments.push(RecordingSegment {
            path,
            start_ns,
            end_ns,
            bytes,
            sha256,
            decoded,
        });
    }
    Ok(segments)
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
    let location = path
        .to_str()
        .ok_or(OutputError::RecordingFinalization)?
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let description = format!(
        "filesrc location=\"{location}\" ! matroskademux name=demux \
         demux. ! queue ! decodebin ! fakesink sync=false \
         demux. ! queue ! decodebin ! fakesink sync=false"
    );
    let pipeline = gst::parse::launch(&description)
        .map_err(|_| OutputError::RecordingFinalization)?
        .downcast::<gst::Pipeline>()
        .map_err(|_| OutputError::RecordingFinalization)?;
    let bus = pipeline.bus().ok_or(OutputError::RecordingFinalization)?;
    pipeline
        .set_state(gst::State::Playing)
        .map_err(|_| OutputError::RecordingFinalization)?;
    let message = bus.timed_pop_filtered(
        gst::ClockTime::from_seconds(30),
        &[gst::MessageType::Eos, gst::MessageType::Error],
    );
    let decoded = message
        .as_ref()
        .is_some_and(|message| matches!(message.view(), gst::MessageView::Eos(_)));
    pipeline
        .set_state(gst::State::Null)
        .map_err(|_| OutputError::RecordingFinalization)?;
    decoded
        .then_some(())
        .ok_or(OutputError::RecordingFinalization)
}
