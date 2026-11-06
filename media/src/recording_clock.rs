use gst::prelude::*;
use gstreamer as gst;
use std::sync::{Arc, Mutex};

use crate::{
    engine_owner::now_nanoseconds,
    protocol::EngineClockMapping,
    recording_provenance::{ProvenanceCollector, ProvenanceSnapshot},
};

pub struct RecordingClock {
    pub engine_run_id: Option<String>,
    pub media_origin_ns: u64,
    pub uncertainty_ns: u64,
    corrections: Arc<Mutex<Option<(i128, i128)>>>,
    provenance: Arc<ProvenanceCollector>,
}

impl RecordingClock {
    pub fn source_capture(
        anchor: &EngineClockMapping,
        engine_run_id: Option<String>,
        provenance: Arc<ProvenanceCollector>,
    ) -> Self {
        Self {
            engine_run_id,
            media_origin_ns: anchor.media_time.value(),
            uncertainty_ns: anchor.uncertainty.value(),
            corrections: Arc::new(Mutex::new(Some((0, 0)))),
            provenance,
        }
    }

    pub const fn set_media_origin(&mut self, media_origin_ns: u64) {
        self.media_origin_ns = media_origin_ns;
    }

    pub fn sample(
        pipeline: &gst::Pipeline,
        anchor: &EngineClockMapping,
        engine_run_id: Option<String>,
        capture_source_provenance: bool,
    ) -> Option<Self> {
        let (_, state, _) = pipeline.state(gst::ClockTime::from_seconds(2));
        if state != gst::State::Playing {
            return None;
        }
        let before = now_nanoseconds().value();
        let running = pipeline.current_running_time()?.nseconds();
        let after = now_nanoseconds().value();
        let wall_midpoint = before.saturating_add(after.saturating_sub(before) / 2);
        let engine_time = if wall_midpoint >= anchor.wall_time.value() {
            anchor
                .media_time
                .value()
                .checked_add(wall_midpoint - anchor.wall_time.value())?
        } else {
            anchor
                .media_time
                .value()
                .checked_sub(anchor.wall_time.value() - wall_midpoint)?
        };
        let clock = Self {
            engine_run_id,
            media_origin_ns: engine_time.checked_sub(running)?,
            uncertainty_ns: anchor
                .uncertainty
                .value()
                .saturating_add(after.saturating_sub(before).div_ceil(2)),
            corrections: Arc::new(Mutex::new(None)),
            provenance: Arc::new(ProvenanceCollector::default()),
        };
        if let Some(video) = pipeline
            .by_name("recording_video_input")
            .and_then(|source| source.static_pad("src"))
        {
            clock.observe_calibration(&video)?;
            if capture_source_provenance {
                clock.provenance.observe(&video)?;
            }
        }
        if let Some(audio) = pipeline
            .by_name("recording_audio_input")
            .and_then(|source| source.static_pad("src"))
            && capture_source_provenance
        {
            clock.provenance.observe(&audio)?;
        }
        Some(clock)
    }

    pub fn calibration(&self) -> Option<(i128, u64)> {
        let (minimum, maximum) = (*self.corrections.lock().ok()?)?;
        let spread = u64::try_from(maximum - minimum).ok()?;
        Some((
            i128::from(self.media_origin_ns) + minimum + i128::from(spread / 2),
            self.uncertainty_ns.saturating_add(spread.div_ceil(2)),
        ))
    }

    pub fn provenance(&self) -> Option<ProvenanceSnapshot> {
        self.provenance.snapshot()
    }

    fn observe_calibration(&self, pad: &gst::Pad) -> Option<()> {
        let run = self.engine_run_id.clone()?;
        let origin = i128::from(self.media_origin_ns);
        let corrections = Arc::clone(&self.corrections);
        pad.add_probe(gst::PadProbeType::BUFFER, move |_, info| {
            if let Some(buffer) = info.buffer()
                && let Some(pts) = buffer.pts()
                && let Some(meta) = buffer
                    .iter_meta::<gst::ReferenceTimestampMeta>()
                    .filter(|meta| {
                        meta.reference().structure(0).is_some_and(|reference| {
                            reference.name() == "timestamp/x-live-production-engine"
                                && reference.get::<&str>("engine-run-id").ok() == Some(run.as_str())
                        })
                    })
                    .last()
                && let Ok(mut offsets) = corrections.lock()
            {
                let correction =
                    i128::from(meta.timestamp().nseconds()) - i128::from(pts.nseconds()) - origin;
                *offsets = Some(
                    offsets.map_or((correction, correction), |(minimum, maximum)| {
                        (minimum.min(correction), maximum.max(correction))
                    }),
                );
            }
            gst::PadProbeReturn::Ok
        })?;
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{engine_frame_fixture::Fixture, protocol::Nanoseconds};

    #[test]
    fn clock_origin_accounts_for_already_elapsed_pipeline_time() -> anyhow::Result<()> {
        let fixture = Fixture::new(10)?;
        for _ in 0..4 {
            let _ = fixture.frame()?;
        }
        let before = now_nanoseconds().value();
        let elapsed = fixture
            .pipeline
            .current_running_time()
            .ok_or_else(|| anyhow::anyhow!("running clock"))?
            .nseconds();
        assert!(elapsed > 100_000_000);
        let anchor = EngineClockMapping {
            media_time: Nanoseconds::new(5_000_000_000),
            wall_time: Nanoseconds::new(before),
            uncertainty: Nanoseconds::new(1_000_000),
        };
        let clock = RecordingClock::sample(&fixture.pipeline, &anchor, Some("run".into()), false)
            .ok_or_else(|| anyhow::anyhow!("recording clock"))?;
        let after = now_nanoseconds().value();
        let expected = 5_000_000_000_u64 - elapsed;
        assert!(clock.media_origin_ns.abs_diff(expected) <= after - before + 1_000_000);
        Ok(())
    }

    #[test]
    fn unavailable_running_time_stays_unmapped() -> anyhow::Result<()> {
        gst::init()?;
        let pipeline = gst::Pipeline::new();
        let anchor = EngineClockMapping {
            media_time: Nanoseconds::new(1),
            wall_time: Nanoseconds::new(1),
            uncertainty: Nanoseconds::new(0),
        };
        assert!(RecordingClock::sample(&pipeline, &anchor, None, false).is_none());
        Ok(())
    }
}
