use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use gst::prelude::*;
use gstreamer as gst;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginAvailability {
    pub name: String,
    pub available: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineOutcome {
    Finalized,
}

pub fn probe_plugins(names: &[&str]) -> Result<Vec<PluginAvailability>> {
    gst::init().context("initialize GStreamer")?;
    Ok(names
        .iter()
        .map(|name| PluginAvailability {
            name: (*name).to_owned(),
            available: gst::ElementFactory::find(name).is_some(),
        })
        .collect())
}

pub fn run_pipeline_for(description: &str, duration: Duration) -> Result<PipelineOutcome> {
    gst::init().context("initialize GStreamer")?;
    let element = gst::parse::launch(description).context("parse GStreamer pipeline")?;
    let pipeline = element
        .downcast::<gst::Pipeline>()
        .map_err(|_| anyhow!("pipeline description did not produce a Pipeline"))?;
    let bus = pipeline.bus().context("pipeline has no bus")?;

    pipeline
        .set_state(gst::State::Playing)
        .map_err(|error| anyhow!("start GStreamer pipeline: {error:?}"))?;

    if let Some(message) = bus.timed_pop_filtered(
        gst::ClockTime::from_nseconds(duration.as_nanos().try_into().unwrap_or(u64::MAX)),
        &[gst::MessageType::Error, gst::MessageType::Eos],
    ) {
        match message.view() {
            gst::MessageView::Eos(_) => {
                pipeline
                    .set_state(gst::State::Null)
                    .map_err(|error| anyhow!("stop GStreamer pipeline: {error:?}"))?;
                return Ok(PipelineOutcome::Finalized);
            }
            gst::MessageView::Error(error) => {
                let _ = pipeline.set_state(gst::State::Null);
                return Err(anyhow!(
                    "GStreamer pipeline failed: {} ({:?})",
                    error.error(),
                    error.debug()
                ));
            }
            _ => {}
        }
    }

    if !pipeline.send_event(gst::event::Eos::new()) {
        let _ = pipeline.set_state(gst::State::Null);
        return Err(anyhow!("GStreamer pipeline rejected EOS"));
    }

    let final_message = bus.timed_pop_filtered(
        gst::ClockTime::from_seconds(15),
        &[gst::MessageType::Error, gst::MessageType::Eos],
    );
    let result = match final_message.as_ref().map(|message| message.view()) {
        Some(gst::MessageView::Eos(_)) => Ok(PipelineOutcome::Finalized),
        Some(gst::MessageView::Error(error)) => Err(anyhow!(
            "GStreamer pipeline failed while finalizing: {} ({:?})",
            error.error(),
            error.debug()
        )),
        Some(_) => Err(anyhow!("unexpected GStreamer bus message while finalizing")),
        None => Err(anyhow!(
            "GStreamer pipeline did not finalize within 15 seconds"
        )),
    };
    pipeline
        .set_state(gst::State::Null)
        .map_err(|error| anyhow!("stop GStreamer pipeline: {error:?}"))?;
    result
}
