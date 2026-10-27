use std::{
    net::UdpSocket,
    time::{Duration, Instant},
};

use live_production_media::{
    engine::MediaEngine,
    engine_source_admission::AdmissionSnapshot,
    protocol::{
        Command, CommandId, CommandKind, EngineEvent, EventKind, Milliseconds, SourceId,
        SourceKind, SourceSnapshot, SourceSpec, TrackId,
    },
};

mod media;
pub use media::*;

pub const PASSPHRASE: &str = "singlePublisherCredential42";

pub fn add_source(engine: &MediaEngine, port: u16) -> anyhow::Result<EngineEvent> {
    engine
        .execute(Command {
            id: CommandId::new("admission-add")?,
            expected_engine_run_id: None,
            kind: CommandKind::AddSource {
                source: SourceSpec {
                    id: SourceId::new("admission-source")?,
                    video_track_id: TrackId::new("admission-video")?,
                    audio_track_id: TrackId::new("admission-audio")?,
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

pub fn source_snapshot(
    engine: &MediaEngine,
    added: &EngineEvent,
    id: &str,
) -> anyhow::Result<SourceSnapshot> {
    let event = engine.execute(Command {
        id: CommandId::new(id)?,
        expected_engine_run_id: Some(added.engine_run_id.clone()),
        kind: CommandKind::QueryRuntime,
    })?;
    let EventKind::RuntimeSnapshot { sources, .. } = event.kind else {
        anyhow::bail!("expected runtime snapshot");
    };
    sources
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("source missing"))
}

pub fn wait_for_admission(
    added: &EngineEvent,
    expected_accepted: u64,
    minimum_rejected: u64,
    expected_generation: u64,
) -> anyhow::Result<AdmissionSnapshot> {
    wait_for_state(added, |state| {
        state.active
            && state.accepted == expected_accepted
            && state.rejected >= minimum_rejected
            && state.generation == expected_generation
    })
}

pub fn wait_for_pending(added: &EngineEvent) -> anyhow::Result<AdmissionSnapshot> {
    wait_for_state(added, |state| !state.active && state.pending_peer.is_some())
}

pub fn wait_for_recovered_vacancy(added: &EngineEvent) -> anyhow::Result<AdmissionSnapshot> {
    wait_for_state(added, |state| {
        !state.active && state.pending_peer.is_none() && state.accepted == 0
    })
}

pub fn free_udp_port() -> anyhow::Result<u16> {
    let socket = UdpSocket::bind(("127.0.0.1", 0))?;
    Ok(socket.local_addr()?.port())
}

fn wait_for_state(
    added: &EngineEvent,
    predicate: impl Fn(&AdmissionSnapshot) -> bool,
) -> anyhow::Result<AdmissionSnapshot> {
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut last = None;
    while Instant::now() < deadline {
        if let Some(state) = live_production_media::engine_source_admission::snapshot(
            &added.engine_run_id,
            &SourceId::new("admission-source")?,
        ) {
            if predicate(&state) {
                return Ok(state);
            }
            last = Some(state);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    anyhow::bail!("admission state did not converge: {last:?}")
}
