use anyhow::Context;
use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app::AppSink;
use live_production_media::{
    engine::{EngineConfig, MediaEngine},
    output::{MediaTap, OutputDefinition, OutputManager, OutputState, RecordingDefinition},
    protocol::{
        Command, CommandId, CommandKind, CompositionPlacement, EngineRunId, EventKind, FitMode,
        NormalizedCrop, OutputId, PlacementContent, PlacementId, ProgramRevision,
    },
    tap::TapChannels,
};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

#[test]
fn recorded_transitions_reconcile_with_program_pts_across_fragments() -> anyhow::Result<()> {
    let engine = MediaEngine::spawn(EngineConfig::new(
        "compositor name=preview_compositor background=black force-live=true ignore-inactive-pads=true ! video/x-raw,width=64,height=64,framerate=30/1 ! fakesink sync=false \
         compositor name=program_compositor background=black force-live=true ignore-inactive-pads=true ! video/x-raw,format=I420,width=64,height=64,framerate=30/1 ! intervideosink name=program_video_tap channel=live-{engine_run_id}-program-video-v1 sync=false async=false \
         audiotestsrc is-live=true wave=silence ! audio/x-raw,rate=48000,channels=2 ! interaudiosink name=program_audio_tap channel=live-{engine_run_id}-program-audio-v1 sync=false async=false",
        std::iter::empty::<(OutputId, &str)>(),
    ))?;
    std::thread::sleep(Duration::from_millis(250));
    let initial = engine.execute(Command {
        id: CommandId::new("mapping-clock")?,
        expected_engine_run_id: None,
        kind: CommandKind::QueryRuntime,
    })?;
    let EventKind::RuntimeSnapshot { clock, .. } = initial.kind else {
        anyhow::bail!("runtime clock unavailable");
    };
    let run = initial.engine_run_id;
    let channels = TapChannels::program(&run);
    let directory = tempfile::tempdir()?;
    let manager = OutputManager::new();
    let output = live_production_media::output::OutputId::new("mapping-recorder")?;
    manager.register(OutputDefinition::Recording(
        RecordingDefinition::new(
            output.clone(),
            MediaTap::new(&channels.video, &channels.audio)?,
            directory.path(),
            Duration::from_secs(1),
        )?
        .with_engine_run_id(&run),
    ))?;
    manager.start_with_clock(&output, Some(clock))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while manager.snapshot(&output)?.actual != OutputState::Running {
        anyhow::ensure!(Instant::now() < deadline, "recorder did not start");
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(1100));
    let green = take_color(&engine, (&run, None), ("green", 0xff00_ff00))?;
    std::thread::sleep(Duration::from_millis(1100));
    let red = take_color(&engine, (&run, Some("green")), ("red", 0xffff_0000))?;
    std::thread::sleep(Duration::from_millis(400));
    manager.stop(&output)?;

    let recordings = manager.recordings(&output)?;
    anyhow::ensure!(recordings.len() >= 3, "insufficient segment boundaries");
    let mut first_green = None;
    let mut first_red = None;
    let mut origins = Vec::new();
    for (index, segment) in recordings.iter().enumerate() {
        let (file_first, frames) = decoded_frames(&segment.path)?;
        let origin = segment
            .media_start_ns
            .context("mapped start")?
            .checked_sub(file_first)
            .context("file PTS exceeds mapped start")?;
        let uncertainty = segment
            .media_uncertainty_ns
            .context("mapping uncertainty")?;
        origins.push(origin);
        for (pts, pixel) in frames {
            let mapped = origin.saturating_add(pts);
            if pixel[1] > 180 && pixel[0] < 40 && first_green.is_none() {
                first_green = Some((index, mapped, uncertainty));
            }
            if pixel[0] > 180 && pixel[1] < 40 && first_red.is_none() {
                first_red = Some((index, mapped, uncertainty));
            }
        }
    }
    for (label, expected, observed, minimum_segment) in [
        ("green", green, first_green.context("green pixels")?, 1),
        ("red", red, first_red.context("red pixels")?, 2),
    ] {
        let (segment, measured, uncertainty) = observed;
        eprintln!(
            "{label}: engine_pts={expected} recorded_pts={measured} uncertainty={uncertainty} segment={segment}"
        );
        anyhow::ensure!(
            segment >= minimum_segment,
            "transition did not cross fragment boundary"
        );
        anyhow::ensure!(
            measured.abs_diff(expected) <= uncertainty + 33_333_334,
            "{label} mapping missed Program PTS: expected={expected} measured={measured} uncertainty={uncertainty}"
        );
    }
    anyhow::ensure!(
        origins
            .windows(2)
            .all(|pair| pair[0].abs_diff(pair[1]) <= 1_000_000),
        "fragment timestamps were rebased: {origins:?}"
    );
    Ok(())
}

fn take_color(
    engine: &MediaEngine,
    program: (&EngineRunId, Option<&str>),
    color: (&str, u32),
) -> anyhow::Result<u64> {
    let (run, expected) = program;
    let (name, argb) = color;
    let revision = ProgramRevision::new(name)?;
    let prepared = engine.execute(Command {
        id: CommandId::new(&format!("prepare-{name}"))?,
        expected_engine_run_id: Some(run.clone()),
        kind: CommandKind::PrepareComposition {
            revision: revision.clone(),
            placements: vec![CompositionPlacement {
                id: PlacementId::new("color")?,
                content: PlacementContent::Solid { argb },
                x: 0,
                y: 0,
                width: 64,
                height: 64,
                crop: NormalizedCrop {
                    left_milli: 0,
                    top_milli: 0,
                    right_milli: 0,
                    bottom_milli: 0,
                },
                fit: FitMode::Stretch,
                opacity_milli: 1000,
                z_order: 0,
            }],
        },
    })?;
    anyhow::ensure!(
        matches!(prepared.kind, EventKind::Applied { .. }),
        "{prepared:?}"
    );
    let taken = engine.execute(Command {
        id: CommandId::new(&format!("take-{name}"))?,
        expected_engine_run_id: Some(run.clone()),
        kind: CommandKind::TakePrepared {
            revision,
            expected_program_revision: expected.map(ProgramRevision::new).transpose()?,
        },
    })?;
    match taken.kind {
        EventKind::Applied {
            media_at: Some(pts),
            ..
        } => Ok(pts.value()),
        _ => anyhow::bail!("Take failed: {taken:?}"),
    }
}

struct Decoder(gst::Pipeline);
impl Drop for Decoder {
    fn drop(&mut self) {
        let _ = self.0.set_state(gst::State::Null);
    }
}

type DecodedFrame = (u64, [u8; 3]);

fn decoded_frames(path: &Path) -> anyhow::Result<(u64, Vec<DecodedFrame>)> {
    let location = path
        .to_str()
        .context("recording path")?
        .replace('"', "\\\"");
    let decoder = Decoder(gst::parse::launch(&format!(
        "filesrc location=\"{location}\" ! matroskademux name=demux \
         demux.video_0 ! queue ! h264parse ! identity name=video_pts ! avdec_h264 ! videoconvert ! video/x-raw,format=RGB ! appsink name=frames max-buffers=4 sync=false \
         demux.audio_0 ! queue ! aacparse ! identity name=audio_pts ! fakesink sync=false"
    ))?.downcast::<gst::Pipeline>().map_err(|_| anyhow::anyhow!("decode pipeline"))?);
    let first = Arc::new(AtomicU64::new(u64::MAX));
    for name in ["video_pts", "audio_pts"] {
        let first = Arc::clone(&first);
        decoder
            .0
            .by_name(name)
            .context("packet probe")?
            .static_pad("src")
            .context("packet pad")?
            .add_probe(gst::PadProbeType::BUFFER, move |_, info| {
                if let Some(pts) = info.buffer().and_then(|buffer| buffer.pts()) {
                    first.fetch_min(pts.nseconds(), Ordering::Relaxed);
                }
                gst::PadProbeReturn::Ok
            })
            .context("packet timestamp observation")?;
    }
    let sink = decoder
        .0
        .by_name("frames")
        .context("frame sink")?
        .downcast::<AppSink>()
        .map_err(|_| anyhow::anyhow!("appsink"))?;
    decoder.0.set_state(gst::State::Playing)?;
    let mut frames = Vec::new();
    while let Some(sample) = sink.try_pull_sample(gst::ClockTime::from_seconds(2)) {
        let buffer = sample.buffer().context("decoded buffer")?;
        let pts = buffer.pts().context("decoded PTS")?.nseconds();
        let mapped = buffer.map_readable()?;
        let caps = sample.caps().context("decoded caps")?;
        let structure = caps.structure(0).context("decoded format")?;
        let width = usize::try_from(structure.get::<i32>("width")?)?;
        let height = usize::try_from(structure.get::<i32>("height")?)?;
        let offset = (height / 2) * (mapped.size() / height) + (width / 2) * 3;
        let pixel = mapped
            .as_slice()
            .get(offset..offset + 3)
            .context("RGB pixel")?;
        frames.push((pts, [pixel[0], pixel[1], pixel[2]]));
    }
    anyhow::ensure!(sink.is_eos(), "decode timed out");
    Ok((first.load(Ordering::Relaxed), frames))
}
