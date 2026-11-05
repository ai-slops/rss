use std::time::{Duration, Instant};

use gstreamer as gst;
use live_production_media::{
    engine::{EngineConfig, MediaEngine},
    output::{
        MediaTap, OutputDefinition, OutputId as ManagedOutputId, OutputManager, OutputState,
        RecordingDefinition, RecordingSegment,
    },
    protocol::{
        AudioMixRoute, Command, CommandId, CommandKind, CompositionPlacement, EngineEvent,
        EventKind, FitMode, Milliseconds, NormalizedCrop, OutputId, PlacementContent, PlacementId,
        ProgramProgress, ProgramRevision, SourceId, SourceKind, SourceSnapshot, SourceSpec,
        TrackCorrection, TrackId,
    },
    tap::TapChannels,
};

mod decode;
mod sender;
pub use decode::decode_recordings;
pub use sender::{free_udp_port, rss_kib, sender};

pub const PASSPHRASE: &str = "driftFixtureCredential42";

pub struct Fixture {
    pub engine: MediaEngine,
    pub added: EngineEvent,
    pub port: u16,
    pub outputs: OutputManager,
    pub output_id: ManagedOutputId,
    pub audio_route_sequence: u64,
    _directory: tempfile::TempDir,
}

#[derive(Debug)]
pub struct RuntimeSample {
    pub source: SourceSnapshot,
    pub program: ProgramProgress,
}

impl Fixture {
    pub fn new() -> anyhow::Result<Self> {
        gst::init()?;
        let port = free_udp_port()?;
        let engine = MediaEngine::spawn(EngineConfig::new(
            program_pipeline(),
            std::iter::empty::<(OutputId, &str)>(),
        ))?;
        let added = add_source(&engine, port)?;
        let audio_route_sequence = apply_audio_route(&engine, &added)?;
        take_source(&engine, &added)?;
        let channels = TapChannels::program(&added.engine_run_id);
        let directory = tempfile::tempdir()?;
        let outputs = OutputManager::new();
        let output_id = ManagedOutputId::new("drift-program-recording")?;
        outputs.register(OutputDefinition::Recording(
            RecordingDefinition::new(
                output_id.clone(),
                MediaTap::new(&channels.video, &channels.audio)?,
                directory.path(),
                Duration::from_secs(5),
            )?
            .with_engine_run_id(&added.engine_run_id),
        ))?;
        outputs.start(&output_id)?;
        wait_for_output(&outputs, &output_id)?;
        Ok(Self {
            engine,
            added,
            port,
            outputs,
            output_id,
            audio_route_sequence,
            _directory: directory,
        })
    }

    pub fn snapshot(&self, id: &str) -> anyhow::Result<RuntimeSample> {
        let event = self.engine.execute(Command {
            id: CommandId::new(id)?,
            expected_engine_run_id: Some(self.added.engine_run_id.clone()),
            kind: CommandKind::QueryRuntime,
        })?;
        let EventKind::RuntimeSnapshot {
            sources, program, ..
        } = event.kind
        else {
            anyhow::bail!("expected runtime snapshot");
        };
        Ok(RuntimeSample {
            source: sources
                .into_iter()
                .next()
                .ok_or_else(|| anyhow::anyhow!("source missing"))?,
            program,
        })
    }

    pub fn apply_timing(&self, id: &str, video_ms: i64, audio_ms: i64) -> anyhow::Result<()> {
        let event = self.engine.execute(Command {
            id: CommandId::new(id)?,
            expected_engine_run_id: Some(self.added.engine_run_id.clone()),
            kind: CommandKind::ApplyTrackTiming {
                program_holdback: Milliseconds::new(500),
                corrections: vec![
                    TrackCorrection {
                        track_id: TrackId::new("drift-video")?,
                        correction: Milliseconds::new(video_ms),
                    },
                    TrackCorrection {
                        track_id: TrackId::new("drift-audio")?,
                        correction: Milliseconds::new(audio_ms),
                    },
                ],
            },
        })?;
        anyhow::ensure!(matches!(
            event.kind,
            EventKind::Applied {
                media_at: Some(_),
                ..
            }
        ));
        Ok(())
    }

    pub fn finish(&self) -> anyhow::Result<Vec<RecordingSegment>> {
        self.outputs.stop(&self.output_id)?;
        wait_for_stopped(&self.outputs, &self.output_id)?;
        self.outputs.recordings(&self.output_id).map_err(Into::into)
    }

    pub fn recording_directory(&self) -> &std::path::Path {
        self._directory.path()
    }
}

fn apply_audio_route(engine: &MediaEngine, added: &EngineEvent) -> anyhow::Result<u64> {
    let event = engine.execute(Command {
        id: CommandId::new("drift-audio-route")?,
        expected_engine_run_id: Some(added.engine_run_id.clone()),
        kind: CommandKind::ApplyAudioMix {
            routes: vec![AudioMixRoute {
                track_id: TrackId::new("drift-audio")?,
                gain_milli_db: 0,
                muted: false,
                routed: true,
            }],
        },
    })?;
    anyhow::ensure!(matches!(event.kind, EventKind::Applied { .. }));
    Ok(event.sequence.value())
}

fn add_source(engine: &MediaEngine, port: u16) -> anyhow::Result<EngineEvent> {
    engine
        .execute(Command {
            id: CommandId::new("drift-add")?,
            expected_engine_run_id: None,
            kind: CommandKind::AddSource {
                source: SourceSpec {
                    id: SourceId::new("drift-source")?,
                    video_track_id: TrackId::new("drift-video")?,
                    audio_track_id: TrackId::new("drift-audio")?,
                    kind: SourceKind::SrtListener {
                        bind_address: "127.0.0.1".to_owned(),
                        port,
                        latency: Milliseconds::new(120),
                        passphrase: PASSPHRASE.to_owned(),
                    },
                },
            },
        })
        .map_err(Into::into)
}

fn take_source(engine: &MediaEngine, added: &EngineEvent) -> anyhow::Result<()> {
    let revision = ProgramRevision::new("drift-scene")?;
    let placement = CompositionPlacement {
        id: PlacementId::new("drift-placement")?,
        content: PlacementContent::Track {
            track_id: TrackId::new("drift-video")?,
        },
        x: 0,
        y: 0,
        width: 960,
        height: 540,
        crop: NormalizedCrop {
            left_milli: 0,
            top_milli: 0,
            right_milli: 0,
            bottom_milli: 0,
        },
        fit: FitMode::Cover,
        opacity_milli: 1_000,
        z_order: 0,
    };
    engine.execute(Command {
        id: CommandId::new("drift-prepare")?,
        expected_engine_run_id: Some(added.engine_run_id.clone()),
        kind: CommandKind::PrepareComposition {
            revision: revision.clone(),
            placements: vec![placement],
        },
    })?;
    let taken = engine.execute(Command {
        id: CommandId::new("drift-take")?,
        expected_engine_run_id: Some(added.engine_run_id.clone()),
        kind: CommandKind::TakePrepared {
            revision,
            expected_program_revision: None,
        },
    })?;
    anyhow::ensure!(matches!(taken.kind, EventKind::Applied { .. }));
    Ok(())
}

fn wait_for_output(manager: &OutputManager, id: &ManagedOutputId) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let snapshot = manager.snapshot(id)?;
        if snapshot.actual == OutputState::Running && snapshot.last_media_at_ns.is_some() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    anyhow::bail!("recording output did not start")
}

fn wait_for_stopped(manager: &OutputManager, id: &ManagedOutputId) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if manager.snapshot(id)?.actual == OutputState::Stopped {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    anyhow::bail!("recording output did not stop")
}

const fn program_pipeline() -> &'static str {
    "compositor name=preview_compositor background=black force-live=true ignore-inactive-pads=true ! video/x-raw,width=960,height=540,framerate=30/1 ! fakesink sync=false \
     compositor name=program_compositor background=black force-live=true ignore-inactive-pads=true ! video/x-raw,width=960,height=540,framerate=30/1 ! tee name=program_video \
     program_video. ! queue max-size-buffers=2 leaky=downstream ! intervideosink name=program_video_tap channel=live-{engine_run_id}-program-video-v1 sync=false async=false \
     program_video. ! queue max-size-buffers=2 leaky=downstream ! fakesink sync=false \
     audiomixer name=program_mixer force-live=true ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! tee name=program_audio \
     program_audio. ! queue max-size-time=200000000 leaky=downstream ! interaudiosink name=program_audio_tap channel=live-{engine_run_id}-program-audio-v1 sync=false async=false \
     program_audio. ! queue max-size-time=200000000 leaky=downstream ! fakesink sync=false"
}
