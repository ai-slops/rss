use crate::{
    engine_composition_transform::{crop_pixels, element_stem, quote, transform_chain},
    protocol::{CompositionPlacement, NormalizedCrop, PlacementContent, TrackId},
};
use gstreamer::{self as gst, prelude::*};
use std::{sync::mpsc, time::Instant};

pub struct CompositionLayerRuntime {
    pipeline: gst::Pipeline,
    bin: gst::Bin,
    compositor: gst::Element,
    pub pad: gst::Pad,
    attached: bool,
}

pub fn build_layer(
    pipeline: &gst::Pipeline,
    compositor: &gst::Element,
    tracks: &std::collections::HashMap<TrackId, String>,
    placement: &CompositionPlacement,
    deadline: Instant,
) -> Result<CompositionLayerRuntime, String> {
    let chain = transform_chain(placement);
    let description = match &placement.content {
        PlacementContent::Track { track_id } => format!(
            "intervideosrc channel={} ! queue max-size-buffers=4 leaky=downstream ! {chain}",
            quote(
                tracks
                    .get(track_id)
                    .ok_or_else(|| format!("unknown video track '{}'", track_id.as_str()))?
            )
        ),
        PlacementContent::Solid { argb } => format!(
            "videotestsrc is-live=true pattern=solid-color foreground-color={argb} ! video/x-raw,width=1280,height=720,framerate=30/1 ! {chain}"
        ),
        PlacementContent::Text {
            text,
            argb,
            font_size,
        } => format!(
            "videotestsrc is-live=true pattern=solid-color foreground-color=0 ! video/x-raw,format=BGRA,width=1280,height=720,framerate=30/1 ! textoverlay wait-text=false text={} color={argb} valignment=center halignment=center font-desc={} ! {chain}",
            quote(text),
            quote(&format!("Sans {font_size}"))
        ),
        PlacementContent::Image { path } => format!(
            "filesrc location={} ! decodebin ! imagefreeze is-live=true ! video/x-raw,framerate=30/1 ! {chain}",
            quote(path)
        ),
    };
    let bin = gst::parse::bin_from_description_with_name(
        &description,
        false,
        &format!(
            "layer_{}_{}_{}",
            element_stem(compositor.name().as_str()),
            element_stem(placement.id.as_str()),
            uuid::Uuid::now_v7().simple()
        ),
    )
    .map_err(|error| format!("build placement: {error}"))?;
    let target = bin
        .by_name("scene_output")
        .and_then(|element| element.static_pad("src"))
        .ok_or_else(|| "placement has no final output".to_owned())?;
    let ghost = gst::GhostPad::builder_with_target(&target)
        .map_err(|error| error.to_string())?
        .name("src")
        .build();
    ghost.set_active(true).map_err(|error| error.to_string())?;
    bin.add_pad(&ghost).map_err(|error| error.to_string())?;
    configure_normalized_crop(&bin, placement.crop)?;
    let pad = compositor
        .request_pad_simple("sink_%u")
        .ok_or_else(|| "compositor rejected placement pad".to_owned())?;
    pad.set_property("alpha", 0.0_f64);
    let mut layer = CompositionLayerRuntime {
        pipeline: pipeline.clone(),
        bin,
        compositor: compositor.clone(),
        pad,
        attached: false,
    };
    pipeline
        .add(&layer.bin)
        .map_err(|error| format!("attach placement: {error}"))?;
    layer.attached = true;
    let output = layer
        .bin
        .static_pad("src")
        .ok_or_else(|| "placement has no output".to_owned())?;
    output
        .link(&layer.pad)
        .map_err(|error| format!("link placement: {error:?}"))?;
    let (ready, prepared) = mpsc::sync_channel(1);
    output
        .add_probe(gst::PadProbeType::BUFFER, move |_, _| {
            let _ = ready.try_send(());
            gst::PadProbeReturn::Remove
        })
        .ok_or_else(|| "could not observe placement readiness".to_owned())?;
    layer
        .bin
        .sync_state_with_parent()
        .map_err(|error| format!("start placement: {error}"))?;
    prepared
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|_| format!("placement '{}' produced no buffer or configured fallback before preparation deadline", placement.id.as_str()))?;
    Ok(layer)
}

fn configure_normalized_crop(bin: &gst::Bin, crop: NormalizedCrop) -> Result<(), String> {
    let element = bin
        .by_name("scene_crop")
        .ok_or_else(|| "placement crop element missing".to_owned())?;
    let input = element
        .static_pad("sink")
        .ok_or_else(|| "crop input pad missing".to_owned())?;
    let element = element.downgrade();
    input
        .add_probe(gst::PadProbeType::EVENT_DOWNSTREAM, move |_, info| {
            let Some(element) = element.upgrade() else {
                return gst::PadProbeReturn::Remove;
            };
            if let Some(event) = info.event()
                && let gst::EventView::Caps(caps) = event.view()
                && let Some(structure) = caps.caps().structure(0)
                && let (Ok(width), Ok(height)) = (
                    structure.get::<i32>("width"),
                    structure.get::<i32>("height"),
                )
            {
                let (left, top, right, bottom) = crop_pixels(crop, width, height);
                for (property, value) in [
                    ("left", left),
                    ("top", top),
                    ("right", right),
                    ("bottom", bottom),
                ] {
                    if element.property::<i32>(property) != value {
                        element.set_property(property, value);
                    }
                }
            }
            gst::PadProbeReturn::Ok
        })
        .ok_or_else(|| "cannot observe input dimensions for normalized crop".to_owned())?;
    Ok(())
}

impl Drop for CompositionLayerRuntime {
    fn drop(&mut self) {
        if let Err(error) = self.bin.set_state(gst::State::Null) {
            gst::warning!(gst::CAT_RUST, "placement stop failed: {error:?}");
        }
        self.compositor.release_request_pad(&self.pad);
        if self.attached
            && let Err(error) = self.pipeline.remove(&self.bin)
        {
            gst::warning!(gst::CAT_RUST, "placement detach failed: {error}");
        }
    }
}
