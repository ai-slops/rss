mod srt_admission_support;

use std::{
    sync::{Arc, Barrier},
    time::Duration,
};

use gstreamer as gst;
use live_production_media::{
    engine::{EngineConfig, MediaEngine},
    protocol::{OutputId, SourceId, TrackId},
    tap::TapChannels,
};
use srt_admission_support::*;

#[test]
fn barrier_started_srt_publishers_admit_one_and_reconnect_advances_generation() -> anyhow::Result<()>
{
    // Given
    gst::init()?;
    let port = free_udp_port()?;
    let engine = MediaEngine::spawn(EngineConfig::new(
        "fakesrc num-buffers=1 ! fakesink",
        std::iter::empty::<(OutputId, &str)>(),
    ))?;
    let added = add_source(&engine, port)?;
    let channel =
        TapChannels::source_video(&added.engine_run_id, &TrackId::new("admission-video")?);
    let (consumer, sink) = consumer(&channel)?;
    consumer.start()?;
    let barrier = Arc::new(Barrier::new(3));
    let red = start_on_barrier(sender(port, 0xffff_0000, 440)?, Arc::clone(&barrier));
    let green = start_on_barrier(sender(port, 0xff00_ff00, 880)?, Arc::clone(&barrier));

    // When
    barrier.wait();
    let red = join_sender(red)?;
    let green = join_sender(green)?;
    let authoritative = wait_for_authoritative_color(&sink)?;
    let first_connected = source_snapshot(&engine, &added, "admission-first-connected")?;
    assert!(first_connected.connected);
    assert_samples_remain(&sink, authoritative, 12)?;
    let blocked = wait_for_admission(&added, 1, 0, 1)?;
    let (winner, loser) = match authoritative {
        Color::Red => (red, green),
        Color::Green => (green, red),
        Color::Blue | Color::Other => anyhow::bail!("unexpected authoritative color"),
    };
    loser.stop()?;
    assert_samples_remain(&sink, authoritative, 6)?;
    winner.stop()?;
    std::thread::sleep(Duration::from_secs(3));
    let disconnected = source_snapshot(&engine, &added, "admission-disconnected")?;
    let reconnect = sender(port, 0xff00_00ff, 660)?;
    reconnect.start()?;
    wait_for_color(&sink, Color::Blue)?;
    let connected = source_snapshot(&engine, &added, "admission-reconnected")?;

    // Then
    assert!(!disconnected.connected);
    assert!(
        disconnected
            .gaps
            .last()
            .is_some_and(|gap| gap.ended_at.is_none())
    );
    assert!(connected.connected);
    assert_eq!(connected.id, SourceId::new("admission-source")?);
    assert_eq!(connected.continuity_generation.value(), 2);
    assert!(
        connected
            .gaps
            .last()
            .is_some_and(|gap| gap.ended_at.is_some())
    );
    let reaccepted = wait_for_admission(&added, 2, 0, 2)?;
    let active_peer = blocked
        .active_peer
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("active peer missing"))?;
    println!(
        "admission_evidence barrier_first={authoritative:?} second_not_accepted=true callback_rejected={} active_peer={} reconnect=blue generation={} source=admission-source track=admission-video gap_closed=true",
        blocked.rejected, active_peer, reaccepted.generation
    );
    reconnect.stop()?;
    consumer.stop()?;
    Ok(())
}

#[test]
fn accepted_disconnect_failed_pending_recovery_advances_video_and_audio_epoch() -> anyhow::Result<()>
{
    // Given
    gst::init()?;
    let port = free_udp_port()?;
    let engine = MediaEngine::spawn(EngineConfig::new(
        "fakesrc num-buffers=1 ! fakesink",
        std::iter::empty::<(OutputId, &str)>(),
    ))?;
    let added = add_source(&engine, port)?;
    let channels = TapChannels::source(
        &added.engine_run_id,
        &TrackId::new("admission-video")?,
        &TrackId::new("admission-audio")?,
    );
    let (video_consumer, video_sink) = consumer(&channels.video)?;
    let (audio_consumer, audio_sink) = audio_consumer(&channels.audio)?;
    video_consumer.start()?;
    audio_consumer.start()?;
    let red = sender(port, 0xffff_0000, 440)?;
    red.start()?;
    wait_for_color_generation(&video_sink, Color::Red, 1)?;
    wait_for_audio_generation(&audio_sink, 1)?;
    let first = source_snapshot(&engine, &added, "admission-red-connected")?;
    assert_eq!(first.continuity_generation.value(), 1);
    red.stop()?;
    std::thread::sleep(Duration::from_secs(3));
    let disconnected = source_snapshot(&engine, &added, "admission-red-disconnected")?;
    assert!(!disconnected.connected);

    // When
    let stream_id = provisioned_stream_id();
    let wrong =
        sender_with_stream_id(port, 0xff00_ff00, 880, "wrongAdmissionSecret99", &stream_id)?;
    wrong.start()?;
    let pending = wait_for_pending(&added)?;
    let recovery_started = std::time::Instant::now();
    wrong.stop()?;
    wait_for_recovered_vacancy(&added)?;
    let recovery_ms = recovery_started.elapsed().as_millis();
    let blue = sender(port, 0xff00_00ff, 660)?;
    blue.start()?;
    wait_for_color_generation(&video_sink, Color::Blue, 2)?;
    wait_for_audio_generation(&audio_sink, 2)?;
    let recovered = source_snapshot(&engine, &added, "admission-blue-connected")?;

    // Then
    let state = wait_for_admission(&added, 1, 0, 2)?;
    assert!(state.active);
    assert_eq!(recovered.continuity_generation.value(), 2);
    assert!(
        recovered
            .gaps
            .last()
            .is_some_and(|gap| gap.ended_at.is_some())
    );
    let pending_peer = pending
        .pending_peer
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("pending peer missing"))?;
    println!(
        "wrong_secret_recovery previous=red pending_peer={} recovery_ms={} valid_retry=blue generation={} video_epoch=2 audio_epoch=2",
        pending_peer, recovery_ms, state.generation
    );
    blue.stop()?;
    video_consumer.stop()?;
    audio_consumer.stop()?;
    Ok(())
}
