use gstreamer::{self as gst, prelude::*};

use crate::protocol::EngineRunId;

pub fn tag(pad: &gst::Pad, run: &EngineRunId) -> Result<(), String> {
    let reference = gst::Caps::builder("timestamp/x-live-production-engine")
        .field("engine-run-id", run.as_str())
        .build();
    pad.add_probe(gst::PadProbeType::BUFFER, move |_, info| {
        if let Some(buffer) = info.buffer_mut()
            && let Some(pts) = buffer.pts()
        {
            let duration = buffer.duration();
            gst::ReferenceTimestampMeta::add(buffer.make_mut(), &reference, pts, duration);
        }
        gst::PadProbeReturn::Ok
    })
    .ok_or_else(|| "cannot preserve engine timestamps at native tap".to_owned())?;
    Ok(())
}
