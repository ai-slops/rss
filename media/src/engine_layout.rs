use std::sync::mpsc;
use std::time::Duration;

use gst::prelude::*;
use gstreamer as gst;

use crate::protocol::ScenePlacement;

pub fn apply_at_frame_boundary(
    pipeline: &gst::Pipeline,
    compositor_name: &str,
    placements: &[ScenePlacement],
) -> Result<(), String> {
    let compositor = pipeline
        .by_name(compositor_name)
        .ok_or_else(|| format!("pipeline has no {compositor_name} element"))?;
    for placement in placements {
        validate_placement(&compositor, placement)?;
    }
    let source_pad = compositor
        .static_pad("src")
        .ok_or_else(|| format!("{compositor_name} has no src pad"))?;
    let compositor_for_probe = compositor;
    let placements_for_probe = placements.to_vec();
    let (applied_sender, applied_receiver) = mpsc::sync_channel(1);
    source_pad
        .add_probe(gst::PadProbeType::BLOCK_DOWNSTREAM, move |_, _| {
            for placement in &placements_for_probe {
                if let Some(pad) =
                    compositor_for_probe.static_pad(&format!("sink_{}", placement.slot))
                {
                    pad.set_property("xpos", placement.x);
                    pad.set_property("ypos", placement.y);
                    pad.set_property("width", placement.width);
                    pad.set_property("height", placement.height);
                    pad.set_property("alpha", f64::from(placement.opacity_milli) / 1_000.0);
                }
            }
            let _ = applied_sender.send(());
            gst::PadProbeReturn::Remove
        })
        .ok_or_else(|| format!("failed to block {compositor_name} output pad"))?;
    applied_receiver
        .recv_timeout(Duration::from_secs(2))
        .map_err(|_| format!("timed out waiting for {compositor_name} frame boundary"))
}

fn validate_placement(compositor: &gst::Element, placement: &ScenePlacement) -> Result<(), String> {
    if placement.width <= 0 || placement.height <= 0 || placement.opacity_milli > 1_000 {
        return Err(format!("invalid placement for slot {}", placement.slot));
    }
    compositor
        .static_pad(&format!("sink_{}", placement.slot))
        .ok_or_else(|| format!("compositor has no sink slot {}", placement.slot))?;
    Ok(())
}
