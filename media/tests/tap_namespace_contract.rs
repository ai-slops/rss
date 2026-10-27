use std::time::Duration;

use gst::prelude::*;
use gstreamer as gst;
use live_production_media::{
    engine::{EngineConfig, MediaEngine},
    protocol::{Command, CommandId, CommandKind, OutputId},
    tap::TapChannels,
};

#[test]
fn parallel_engines_feed_only_their_run_scoped_program_taps() -> anyhow::Result<()> {
    // Given
    gst::init()?;
    let first = engine(320, 180, "ball")?;
    let second = engine(640, 360, "smpte")?;
    let first_run = run_id(&first, "first-run")?;
    let second_run = run_id(&second, "second-run")?;
    let first_channels = TapChannels::program(&first_run);
    let second_channels = TapChannels::program(&second_run);
    let first_consumer = consumer(&first_channels.video, 320, 180)?;
    let second_consumer = consumer(&second_channels.video, 640, 360)?;

    // When
    first_consumer.set_state(gst::State::Playing)?;
    second_consumer.set_state(gst::State::Playing)?;
    std::thread::sleep(Duration::from_millis(500));

    // Then
    assert_ne!(first_channels, second_channels);
    assert!(rendered(&first_consumer) > 0);
    assert!(rendered(&second_consumer) > 0);
    first_consumer.set_state(gst::State::Null)?;
    second_consumer.set_state(gst::State::Null)?;
    Ok(())
}

fn engine(width: u32, height: u32, pattern: &str) -> anyhow::Result<MediaEngine> {
    let description = format!(
        "videotestsrc is-live=true pattern={pattern} ! video/x-raw,width={width},height={height},framerate=30/1 ! intervideosink name=program_video_tap channel=live-{{engine_run_id}}-program-video-v1 sync=false async=false \
         audiotestsrc is-live=true ! audio/x-raw,rate=48000,channels=2 ! interaudiosink name=program_audio_tap channel=live-{{engine_run_id}}-program-audio-v1 sync=false async=false"
    );
    MediaEngine::spawn(EngineConfig::new(
        &description,
        std::iter::empty::<(OutputId, &str)>(),
    ))
    .map_err(Into::into)
}

fn run_id(
    engine: &MediaEngine,
    command_id: &str,
) -> anyhow::Result<live_production_media::protocol::EngineRunId> {
    Ok(engine
        .execute(Command {
            id: CommandId::new(command_id)?,
            expected_engine_run_id: None,
            kind: CommandKind::QueryRuntime,
        })?
        .engine_run_id)
}

fn consumer(channel: &str, width: u32, height: u32) -> anyhow::Result<gst::Pipeline> {
    let description = format!(
        "intervideosrc channel=\"{channel}\" timeout=1000000000 ! video/x-raw,width={width},height={height},framerate=30/1 ! fakesink name=consumer sync=false"
    );
    gst::parse::launch(&description)?
        .downcast::<gst::Pipeline>()
        .map_err(|_| anyhow::anyhow!("consumer description did not create a pipeline"))
}

fn rendered(pipeline: &gst::Pipeline) -> u64 {
    pipeline
        .by_name("consumer")
        .map(|sink| sink.property::<gst::Structure>("stats"))
        .and_then(|stats| stats.get::<u64>("rendered").ok())
        .unwrap_or_default()
}
