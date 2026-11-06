use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;

use crate::source_capture::SourceCaptureFault;

pub fn app_source_at_limit(source: &gst_app::AppSrc) -> bool {
    let reached = |current, maximum| {
        limit_reached(
            source.property::<u64>(current),
            source.property::<u64>(maximum),
        )
    };
    reached("current-level-buffers", "max-buffers")
        || reached("current-level-bytes", "max-bytes")
        || reached("current-level-time", "max-time")
}

pub const fn limit_reached(current: u64, maximum: u64) -> bool {
    maximum != 0 && current >= maximum
}

pub const fn fault_message(fault: SourceCaptureFault) -> &'static str {
    match fault {
        SourceCaptureFault::Closed => "source capture closed",
        SourceCaptureFault::InvalidCapture => "source capture omitted required provenance",
        SourceCaptureFault::Overflow => "source capture queue overflowed",
    }
}
