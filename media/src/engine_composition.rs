use std::{sync::mpsc, time::Duration};

use gst::prelude::*;
use gstreamer as gst;

use crate::engine_composition_transform::{
    element_stem, quote, transform_chain, validate_placements,
};
use crate::engine_owner::OwnerState;
use crate::protocol::{CompositionPlacement, EventKind, PlacementContent, ProgramRevision};

const SOURCE_WIDTH: i32 = 1280;
const SOURCE_HEIGHT: i32 = 720;

pub struct CompositionLayerRuntime {
    bin: gst::Bin,
    compositor: gst::Element,
    compositor_pad: gst::Pad,
}

impl OwnerState {
    pub(super) fn prepare_composition(
        &mut self,
        revision: &ProgramRevision,
        placements: &[CompositionPlacement],
    ) -> EventKind {
        match replace_layers(
            &self.pipeline,
            "preview_compositor",
            &self.video_tees,
            placements,
            &mut self.preview_layers,
        ) {
            Ok(media_at) => {
                self.prepared = None;
                self.prepared_composition = Some((revision.clone(), placements.to_vec()));
                self.applied_media(false, media_at)
            }
            Err(reason) => EventKind::Failed { reason },
        }
    }

    pub(super) fn take_prepared_composition(
        &mut self,
        revision: &ProgramRevision,
        expected_program_revision: Option<&ProgramRevision>,
    ) -> EventKind {
        let Some((prepared_revision, placements)) = self.prepared_composition.as_ref() else {
            return EventKind::Rejected {
                reason: "no prepared Preview revision".to_owned(),
            };
        };
        if prepared_revision != revision {
            return EventKind::Rejected {
                reason: "prepared Preview revision mismatch".to_owned(),
            };
        }
        if expected_program_revision != self.program_revision.as_ref() {
            return EventKind::Rejected {
                reason: "program revision mismatch".to_owned(),
            };
        }
        match replace_layers(
            &self.pipeline,
            "program_compositor",
            &self.video_tees,
            placements,
            &mut self.program_layers,
        ) {
            Ok(media_at) => {
                self.program_revision = Some(revision.clone());
                self.applied_media(false, media_at)
            }
            Err(reason) => EventKind::Failed { reason },
        }
    }
}

fn replace_layers(
    pipeline: &gst::Pipeline,
    compositor_name: &str,
    track_tees: &std::collections::HashMap<crate::protocol::TrackId, String>,
    placements: &[CompositionPlacement],
    current: &mut Vec<CompositionLayerRuntime>,
) -> Result<crate::protocol::Nanoseconds, String> {
    validate_placements(placements)?;
    let compositor = pipeline
        .by_name(compositor_name)
        .ok_or_else(|| format!("pipeline has no {compositor_name} element"))?;
    let mut next = Vec::with_capacity(placements.len());
    for placement in placements {
        match build_layer(pipeline, &compositor, track_tees, placement) {
            Ok(layer) => next.push(layer),
            Err(reason) => {
                for layer in next {
                    let _ = layer.detach(pipeline);
                }
                return Err(reason);
            }
        }
    }
    let media_at = apply_swap_at_boundary(&compositor, current, &next, placements)?;
    for layer in current.drain(..) {
        layer.detach(pipeline)?;
    }
    *current = next;
    Ok(media_at)
}

fn build_layer(
    pipeline: &gst::Pipeline,
    compositor: &gst::Element,
    track_tees: &std::collections::HashMap<crate::protocol::TrackId, String>,
    placement: &CompositionPlacement,
) -> Result<CompositionLayerRuntime, String> {
    let stem = element_stem(placement.id.as_str());
    let chain = transform_chain(placement);
    let description = match &placement.content {
        PlacementContent::Track { track_id } => {
            let channel = track_tees
                .get(track_id)
                .ok_or_else(|| format!("unknown video track '{}'", track_id.as_str()))?;
            format!(
                "intervideosrc channel={} ! queue max-size-buffers=4 leaky=downstream ! {chain}",
                quote(channel)
            )
        }
        PlacementContent::Solid { argb } => format!(
            "videotestsrc is-live=true pattern=solid-color foreground-color={argb} ! video/x-raw,width={SOURCE_WIDTH},height={SOURCE_HEIGHT},framerate=30/1 ! {chain}"
        ),
        PlacementContent::Text {
            text,
            argb,
            font_size,
        } => format!(
            "videotestsrc is-live=true pattern=black ! video/x-raw,width={SOURCE_WIDTH},height={SOURCE_HEIGHT},framerate=30/1 ! textoverlay text={} color={argb} valignment=center halignment=center font-desc={} ! {chain}",
            quote(text),
            quote(&format!("Sans {font_size}")),
        ),
        PlacementContent::Image { path } => format!(
            "filesrc location={} ! decodebin ! imagefreeze is-live=true ! {chain}",
            quote(path)
        ),
    };
    let bin = gst::parse::bin_from_description_with_name(
        &description,
        true,
        &format!("layer_{}_{stem}", element_stem(compositor.name().as_str())),
    )
    .map_err(|error| format!("build placement '{}': {error}", placement.id.as_str()))?;
    pipeline
        .add(&bin)
        .map_err(|error| format!("attach placement '{}': {error}", placement.id.as_str()))?;
    let compositor_pad = compositor.request_pad_simple("sink_%u").ok_or_else(|| {
        format!(
            "{compositor_name} rejected a placement pad",
            compositor_name = compositor.name()
        )
    })?;
    let output = bin
        .static_pad("src")
        .ok_or_else(|| format!("placement '{}' has no output", placement.id.as_str()))?;
    output
        .link(&compositor_pad)
        .map_err(|error| format!("link placement '{}': {error:?}", placement.id.as_str()))?;
    compositor_pad.set_property("alpha", 0.0_f64);
    bin.sync_state_with_parent()
        .map_err(|error| format!("start placement '{}': {error}", placement.id.as_str()))?;
    Ok(CompositionLayerRuntime {
        bin,
        compositor: compositor.clone(),
        compositor_pad,
    })
}

fn apply_swap_at_boundary(
    compositor: &gst::Element,
    current: &[CompositionLayerRuntime],
    next: &[CompositionLayerRuntime],
    placements: &[CompositionPlacement],
) -> Result<crate::protocol::Nanoseconds, String> {
    let output = compositor
        .static_pad("src")
        .ok_or_else(|| format!("{} has no src pad", compositor.name()))?;
    let old_pads: Vec<_> = current
        .iter()
        .map(|layer| layer.compositor_pad.clone())
        .collect();
    let changes: Vec<_> = next
        .iter()
        .zip(placements)
        .map(|(layer, placement)| (layer.compositor_pad.clone(), placement.clone()))
        .collect();
    let (sender, receiver) = mpsc::sync_channel(1);
    output
        .add_probe(
            gst::PadProbeType::BLOCK_DOWNSTREAM | gst::PadProbeType::BUFFER,
            move |_, info| {
                for pad in &old_pads {
                    pad.set_property("alpha", 0.0_f64);
                }
                for (pad, placement) in &changes {
                    pad.set_property("xpos", placement.x);
                    pad.set_property("ypos", placement.y);
                    pad.set_property("alpha", f64::from(placement.opacity_milli) / 1_000.0);
                    pad.set_property("zorder", placement.z_order);
                }
                let media_at = info
                    .buffer()
                    .and_then(|buffer| buffer.pts())
                    .map_or(0, |pts| pts.nseconds());
                let _ = sender.send(crate::protocol::Nanoseconds::new(media_at));
                gst::PadProbeReturn::Remove
            },
        )
        .ok_or_else(|| format!("failed to block {} output", compositor.name()))?;
    receiver
        .recv_timeout(Duration::from_secs(2))
        .map_err(|_| format!("timed out waiting for {} frame boundary", compositor.name()))
}

impl CompositionLayerRuntime {
    fn detach(self, pipeline: &gst::Pipeline) -> Result<(), String> {
        self.bin
            .set_state(gst::State::Null)
            .map_err(|error| format!("stop placement: {error:?}"))?;
        self.compositor.release_request_pad(&self.compositor_pad);
        pipeline
            .remove(&self.bin)
            .map_err(|error| format!("detach placement: {error}"))
    }
}
