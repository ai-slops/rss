use gstreamer::{self as gst, prelude::*};

pub fn attach(pipeline: &gst::Pipeline) -> Result<(), String> {
    for name in ["preview_compositor", "program_compositor"] {
        let Some(compositor) = pipeline.by_name(name) else {
            continue;
        };
        if !compositor.sink_pads().is_empty() {
            continue;
        }
        let source = gst::parse::bin_from_description_with_name(
            "videotestsrc is-live=true pattern=black ! video/x-raw,format=I420,width=16,height=16,framerate=30/1 ! queue max-size-buffers=2 leaky=downstream",
            true,
            &format!("clock_seed_{name}"),
        )
        .map_err(|error| format!("build compositor clock input: {error}"))?;
        pipeline
            .add(&source)
            .map_err(|error| format!("attach compositor clock input: {error}"))?;
        let input = compositor
            .request_pad_simple("sink_%u")
            .ok_or_else(|| "compositor rejected its clock input".to_owned())?;
        input.set_property("alpha", 0.0_f64);
        source
            .static_pad("src")
            .ok_or_else(|| "compositor clock input has no source pad".to_owned())?
            .link(&input)
            .map_err(|error| format!("link compositor clock input: {error:?}"))?;
    }
    Ok(())
}
