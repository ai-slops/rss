use gst::prelude::*;
use gstreamer as gst;
use num_traits::ToPrimitive;
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use crate::engine_source_graph::{element_stem, quote};
use crate::protocol::{EngineRunId, SourceSpec};
use crate::tap::TapChannels;

pub struct AudioMixRuntime {
    bin: gst::Bin,
    mixer: gst::Element,
    mixer_pad: gst::Pad,
    peak: Arc<AtomicU32>,
}

pub struct RouteMeters {
    pub input: Option<i32>,
    pub post: Option<i32>,
    pub clipping: Option<bool>,
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
            "interaudiosrc channel={} ! queue max-size-time=200000000 leaky=downstream ! audioconvert ! audioresample ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! identity name=audio_output",
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
        let peak = Arc::new(AtomicU32::new(u32::MAX));
        let observed = Arc::clone(&peak);
        output
            .add_probe(gst::PadProbeType::BUFFER, move |_, info| {
                let value = info
                    .buffer()
                    .and_then(|buffer| buffer.map_readable().ok())
                    .map_or(u32::MAX, |bytes| {
                        bytes
                            .as_slice()
                            .chunks_exact(2)
                            .map(|sample| {
                                u32::from(i16::from_le_bytes([sample[0], sample[1]]).unsigned_abs())
                            })
                            .max()
                            .unwrap_or(0)
                    });
                observed.store(value, Ordering::Relaxed);
                gst::PadProbeReturn::Ok
            })
            .ok_or_else(|| "cannot observe source PCM level".to_owned())?;
        let mixer_pad = mixer
            .request_pad_simple("sink_%u")
            .ok_or_else(|| "program mixer rejected source audio route".to_owned())?;
        mixer_pad.set_property("mute", true);
        output
            .link(&mixer_pad)
            .map_err(|error| format!("link source audio route: {error:?}"))?;
        bin.sync_state_with_parent()
            .map_err(|error| format!("start source audio route: {error}"))?;
        Ok(Some(Self {
            bin,
            mixer,
            mixer_pad,
            peak,
        }))
    }

    pub fn mixer_pad_name(&self) -> String {
        self.mixer_pad.name().to_string()
    }

    pub fn restore_mix(&self, route: &crate::protocol::AudioMixRoute) {
        self.mixer_pad.set_property(
            "volume",
            10.0_f64.powf(f64::from(route.gain_milli_db) / 20_000.0),
        );
        self.mixer_pad
            .set_property("mute", route.muted || !route.routed);
    }

    pub fn meters(&self) -> RouteMeters {
        let peak = self.peak.load(Ordering::Relaxed);
        if peak == u32::MAX {
            return RouteMeters {
                input: None,
                post: None,
                clipping: None,
            };
        }
        let volume = self.mixer_pad.property::<f64>("volume");
        let muted = self.mixer_pad.property::<bool>("mute");
        let input = level_db(peak, 1.0);
        let post = level_db(peak, if muted { 0.0 } else { volume });
        RouteMeters {
            input,
            post,
            clipping: Some(peak >= 32760 || post.is_some_and(|db| db >= 0)),
        }
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

fn level_db(peak: u32, volume: f64) -> Option<i32> {
    if peak == 0 || volume == 0.0 {
        return Some(-120_000);
    }
    (20000.0 * (f64::from(peak) * volume / 32768.0).log10())
        .round()
        .to_i32()
}

#[cfg(test)]
#[path = "engine_source_audio_tests.rs"]
mod tests;
