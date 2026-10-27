use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use live_production_media::pipeline::{
    ProgramPipelineConfig, clean_pipeline_description, program_pipeline_description,
    rtmps_output_description, srt_input_description, timing_fixture_description,
    webrtc_preview_description,
};
use live_production_media::protocol::Milliseconds;
use live_production_media::runner::{probe_plugins, run_pipeline_for};

const STAGE_A_PLUGINS: &[&str] = &[
    "compositor",
    "audiomixer",
    "splitmuxsink",
    "x264enc",
    "voaacenc",
    "matroskamux",
    "srtsrc",
    "rtmpsink",
    "webrtcsink",
];

#[derive(Debug, Parser)]
#[command(name = "live-production-media")]
#[command(about = "Standalone GStreamer media owner and Stage A fixture")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Describe {
        #[arg(value_enum)]
        path: DescriptionPath,
        #[arg(long)]
        uri: Option<String>,
    },
    Probe,
    StageA {
        #[arg(long, default_value = "var/media-stage-a")]
        output_dir: PathBuf,
        #[arg(long, default_value_t = 6)]
        duration_seconds: u64,
        #[arg(long, default_value_t = 500)]
        holdback_ms: i64,
        #[arg(long, default_value_t = -250)]
        video_correction_ms: i64,
        #[arg(long, default_value_t = 125)]
        audio_correction_ms: i64,
    },
    WebrtcGate {
        #[arg(long, default_value_t = 5)]
        duration_seconds: u64,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum DescriptionPath {
    Rtmps,
    Srt,
    Webrtc,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Describe { path, uri } => describe(path, uri.as_deref()),
        Command::Probe => {
            let report = probe_plugins(STAGE_A_PLUGINS)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            Ok(())
        }
        Command::StageA {
            output_dir,
            duration_seconds,
            holdback_ms,
            video_correction_ms,
            audio_correction_ms,
        } => run_stage_a(
            &output_dir,
            Duration::from_secs(duration_seconds),
            Milliseconds::new(holdback_ms),
            Milliseconds::new(video_correction_ms),
            Milliseconds::new(audio_correction_ms),
        ),
        Command::WebrtcGate { duration_seconds } => {
            run_pipeline_for(
                webrtc_preview_description(),
                Duration::from_secs(duration_seconds),
            )?;
            println!(
                "{}",
                serde_json::json!({
                    "duration_ms": Duration::from_secs(duration_seconds).as_millis().to_string(),
                    "signalling_port": 8443,
                    "status": "finalized",
                    "web_port": 8080
                })
            );
            Ok(())
        }
    }
}

fn describe(path: DescriptionPath, uri: Option<&str>) -> Result<()> {
    let description = match path {
        DescriptionPath::Rtmps => {
            rtmps_output_description(uri.context("--uri is required for the RTMPS description")?)
        }
        DescriptionPath::Srt => {
            srt_input_description(uri.context("--uri is required for the SRT description")?)
        }
        DescriptionPath::Webrtc => webrtc_preview_description().to_owned(),
    };
    println!("{description}");
    Ok(())
}

fn run_stage_a(
    output_dir: &Path,
    duration: Duration,
    holdback: Milliseconds,
    video_correction: Milliseconds,
    audio_correction: Milliseconds,
) -> Result<()> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("create Stage A output directory {}", output_dir.display()))?;
    let program_pattern = output_dir.join("program-%05d.mkv");
    let clean_pattern = output_dir.join("clean-%05d.mkv");
    let timing_evidence = output_dir.join("timing-250ms.mkv");
    let program = program_pipeline_description(&ProgramPipelineConfig::synthetic(
        holdback,
        video_correction,
        audio_correction,
        &program_pattern,
    ))?;
    let clean = clean_pipeline_description(&clean_pattern);
    let timing = timing_fixture_description(&timing_evidence, Milliseconds::new(250));

    let program_thread = thread::spawn(move || run_pipeline_for(&program, duration));
    let clean_thread = thread::spawn(move || run_pipeline_for(&clean, duration));
    let timing_thread = thread::spawn(move || run_pipeline_for(&timing, duration));
    program_thread
        .join()
        .map_err(|_| anyhow!("Program pipeline thread panicked"))??;
    clean_thread
        .join()
        .map_err(|_| anyhow!("clean pipeline thread panicked"))??;
    timing_thread
        .join()
        .map_err(|_| anyhow!("timing pipeline thread panicked"))??;

    println!(
        "{}",
        serde_json::json!({
            "clean_pattern": clean_pattern,
            "duration_ms": duration.as_millis().to_string(),
            "program_pattern": program_pattern,
            "status": "finalized",
            "timing_evidence": timing_evidence
        })
    );
    Ok(())
}
