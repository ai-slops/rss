use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use gst::prelude::*;
use gstreamer as gst;
use serde::{Deserialize, Serialize};

use crate::output::{
    DestinationUrl, MediaTap, OutputDefinition, OutputId, OutputManager, OutputState,
    PROGRAM_AUDIO_CHANNEL, PROGRAM_VIDEO_CHANNEL,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GateConfig {
    duration_seconds: u64,
    destinations: Vec<GateDestination>,
    #[serde(default)]
    expected_unavailable: Vec<String>,
    #[serde(default)]
    ready_path: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GateDestination {
    id: String,
    url: String,
}

#[derive(Serialize)]
pub struct GateReport {
    destinations: Vec<DestinationReport>,
}

#[derive(Serialize)]
struct DestinationReport {
    id: String,
    actual: OutputState,
    media_observed: bool,
}

pub fn run(config_path: &Path) -> Result<GateReport> {
    let config: GateConfig = serde_json::from_slice(
        &fs::read(config_path).with_context(|| format!("read {}", config_path.display()))?,
    )
    .context("parse output gate configuration")?;
    if config.destinations.len() < 2 {
        bail!("output gate requires at least two destinations");
    }
    gst::init().context("initialize GStreamer")?;
    let producer = producer()?;
    producer
        .set_state(gst::State::Playing)
        .map_err(|error| anyhow::anyhow!("start Program tap producer: {error:?}"))?;
    let manager = OutputManager::new();
    let tap = MediaTap::new(PROGRAM_VIDEO_CHANNEL, PROGRAM_AUDIO_CHANNEL)?;
    let mut ids = Vec::with_capacity(config.destinations.len());
    for destination in config.destinations {
        let id = OutputId::new(&destination.id)?;
        manager.register(OutputDefinition::rtmps(
            id.clone(),
            tap.clone(),
            DestinationUrl::new(&destination.url)?,
        ))?;
        manager.start(&id)?;
        ids.push(id);
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline
        && ids.iter().any(|id| {
            manager
                .snapshot(id)
                .is_ok_and(|snapshot| snapshot.last_media_at_ns.is_none())
        })
    {
        thread::sleep(Duration::from_millis(25));
    }
    if ids.iter().any(|id| {
        manager
            .snapshot(id)
            .is_ok_and(|snapshot| snapshot.last_media_at_ns.is_none())
    }) {
        bail!("destinations did not begin sending within ten seconds");
    }
    if let Some(path) = &config.ready_path {
        fs::write(path, b"ready\n")
            .with_context(|| format!("write readiness marker {}", path.display()))?;
    }
    thread::sleep(Duration::from_secs(config.duration_seconds));
    let mut reports = Vec::with_capacity(ids.len());
    for id in &ids {
        let snapshot = manager.snapshot(id)?;
        reports.push(DestinationReport {
            id: id.as_str().to_owned(),
            actual: snapshot.actual,
            media_observed: snapshot.last_media_at_ns.is_some(),
        });
    }
    for id in &ids {
        manager.stop(id)?;
    }
    producer
        .set_state(gst::State::Null)
        .map_err(|error| anyhow::anyhow!("stop Program tap producer: {error:?}"))?;
    for report in &reports {
        let expected_unavailable = config.expected_unavailable.contains(&report.id);
        let expected_state = if expected_unavailable {
            matches!(report.actual, OutputState::Retrying | OutputState::Starting)
        } else {
            report.actual == OutputState::Running
        };
        if !expected_state || !report.media_observed {
            bail!(
                "destination '{}' did not reach its expected state: actual={:?}, media_observed={}",
                report.id,
                report.actual,
                report.media_observed
            );
        }
    }
    Ok(GateReport {
        destinations: reports,
    })
}

fn producer() -> Result<gst::Pipeline> {
    let description = format!(
        "videotestsrc is-live=true pattern=ball ! video/x-raw,width=640,height=360,framerate=30/1 ! intervideosink channel={PROGRAM_VIDEO_CHANNEL} sync=false \
         audiotestsrc is-live=true wave=ticks ! audioconvert ! audioresample ! audio/x-raw,rate=48000,channels=2 ! interaudiosink channel={PROGRAM_AUDIO_CHANNEL} sync=false"
    );
    gst::parse::launch(&description)
        .context("parse Program tap producer")?
        .downcast::<gst::Pipeline>()
        .map_err(|_| anyhow::anyhow!("Program tap description did not produce a pipeline"))
}
