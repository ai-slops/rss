use gst::prelude::*;
use gstreamer as gst;

use crate::engine_source_graph::{element_stem, quote};
use crate::protocol::{EngineRunId, SourceSpec};
use crate::tap::TapChannels;

pub struct AudioMixRuntime {
    bin: gst::Bin,
    mixer: gst::Element,
    mixer_pad: gst::Pad,
}

impl AudioMixRuntime {
    pub fn build(
        pipeline: &gst::Pipeline,
        spec: &SourceSpec,
        engine_run_id: &EngineRunId,
    ) -> Result<Option<Self>, String> {
        let Some(mixer) = pipeline.by_name("program_mixer") else {
            return Ok(None);
        };
        let description = format!(
            "interaudiosrc channel={} ! queue max-size-time=200000000 leaky=downstream ! audioconvert ! audioresample",
            quote(
                &TapChannels::source(engine_run_id, &spec.video_track_id, &spec.audio_track_id,)
                    .audio,
            )
        );
        let bin = gst::parse::bin_from_description_with_name(
            &description,
            true,
            &format!("audio_mix_{}", element_stem(spec.id.as_str())),
        )
        .map_err(|error| format!("build source audio route: {error}"))?;
        pipeline
            .add(&bin)
            .map_err(|error| format!("attach source audio route: {error}"))?;
        let output = bin
            .static_pad("src")
            .ok_or_else(|| "source audio route has no output".to_owned())?;
        let mixer_pad = mixer
            .request_pad_simple("sink_%u")
            .ok_or_else(|| "program mixer rejected source audio route".to_owned())?;
        output
            .link(&mixer_pad)
            .map_err(|error| format!("link source audio route: {error:?}"))?;
        bin.sync_state_with_parent()
            .map_err(|error| format!("start source audio route: {error}"))?;
        Ok(Some(Self {
            bin,
            mixer,
            mixer_pad,
        }))
    }

    pub fn mixer_pad_name(&self) -> String {
        self.mixer_pad.name().to_string()
    }

    pub fn detach(self, pipeline: &gst::Pipeline) -> Result<(), String> {
        self.bin
            .set_state(gst::State::Null)
            .map_err(|error| format!("stop source audio route: {error:?}"))?;
        self.mixer.release_request_pad(&self.mixer_pad);
        pipeline
            .remove(&self.bin)
            .map_err(|error| format!("detach source audio route: {error}"))
    }
}
