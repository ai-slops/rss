use crate::protocol::CompositionPlacement;
use gstreamer::{self as gst, prelude::*};

#[derive(Clone)]
pub struct PadValues {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    alpha: f64,
    z_order: u32,
}

impl PadValues {
    pub fn read(pad: &gst::Pad) -> Self {
        Self {
            x: pad.property("xpos"),
            y: pad.property("ypos"),
            width: pad.property("width"),
            height: pad.property("height"),
            alpha: pad.property("alpha"),
            z_order: pad.property("zorder"),
        }
    }

    pub fn write(&self, pad: &gst::Pad) {
        pad.set_property("xpos", self.x);
        pad.set_property("ypos", self.y);
        pad.set_property("width", self.width);
        pad.set_property("height", self.height);
        pad.set_property("alpha", self.alpha);
        pad.set_property("zorder", self.z_order);
    }
}

#[derive(Clone)]
pub struct PadChange {
    pub pad: gst::Pad,
    pub before: PadValues,
    pub after: PadValues,
}

impl PadChange {
    pub fn hidden(pad: gst::Pad) -> Self {
        let before = PadValues::read(&pad);
        let mut after = before.clone();
        after.alpha = 0.0;
        Self { pad, before, after }
    }

    pub fn placement(pad: gst::Pad, placement: &CompositionPlacement) -> Self {
        Self {
            before: PadValues::read(&pad),
            pad,
            after: PadValues {
                x: placement.x,
                y: placement.y,
                width: placement.width,
                height: placement.height,
                alpha: f64::from(placement.opacity_milli) / 1000.0,
                z_order: placement.z_order,
            },
        }
    }
}
