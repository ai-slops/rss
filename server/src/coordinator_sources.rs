use live_production_media::protocol::{
    Milliseconds, SourceId, SourceKind, SourceSpec, SyntheticPattern, TrackId,
};

use crate::{
    coordinator::Owner,
    error::{AppError, Result},
    model::{Id, Project, Scene, Source, SourceAdapter, Track, TrackKind},
    snapshot::{SourceSnapshot as ApiSourceSnapshot, TrackSnapshot},
    validation,
};

impl Owner {
    pub(crate) fn source_publishing_endpoint(&self, id: &str) -> Result<serde_json::Value> {
        let source = self
            .project
            .sources
            .iter()
            .find(|source| source.id.0 == id)
            .ok_or_else(|| AppError::Invalid("Source no longer exists.".into()))?;
        let SourceAdapter::Srt {
            bind_address,
            port,
            buffer_ms,
        } = &source.adapter
        else {
            return Err(AppError::Invalid(
                "Synthetic sources have no publishing endpoint.".into(),
            ));
        };
        let passphrase = self
            .store
            .ingest_credential(id)
            .map_err(AppError::Internal)?;
        Ok(serde_json::json!({
            "sourceId": id,
            "protocol": "srt",
            "bindAddress": bind_address,
            "port": *port,
            "bufferMs": *buffer_ms,
            "passphrase": passphrase,
            "streamId": live_production_media::engine_source_admission::publisher_stream_id(&passphrase),
            "pbkeylen": 16
        }))
    }
}

pub(crate) fn create_source(
    project: &Project,
    name: &str,
    adapter: &SourceAdapter,
) -> Result<Source> {
    validation::name(name)?;
    let adapter = unique_adapter(project, adapter)?;
    Ok(Source {
        id: Id::new(),
        name: name.to_owned(),
        enabled: true,
        adapter,
        tracks: vec![
            Track {
                id: Id::new(),
                kind: TrackKind::Video,
                correction_ms: 0,
            },
            Track {
                id: Id::new(),
                kind: TrackKind::Audio,
                correction_ms: 0,
            },
        ],
        record_on_connect: true,
    })
}

pub(crate) fn source_spec(source: &Source, credential: &str) -> Result<SourceSpec> {
    let video_track = source
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Video)
        .ok_or_else(|| AppError::Invalid("Source has no video track.".into()))?;
    let audio_track = source
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Audio)
        .ok_or_else(|| AppError::Invalid("Source has no audio track.".into()))?;
    let kind = match &source.adapter {
        SourceAdapter::Synthetic {
            pattern,
            frequency_hz,
        } => SourceKind::Synthetic {
            pattern: synthetic_pattern(pattern)?,
            frequency_hz: *frequency_hz,
        },
        SourceAdapter::Srt {
            bind_address,
            port,
            buffer_ms,
        } => SourceKind::SrtListener {
            bind_address: bind_address.clone(),
            port: *port,
            latency: Milliseconds::new(i64::from(*buffer_ms)),
            passphrase: credential.to_owned(),
        },
    };
    Ok(SourceSpec {
        id: source_id(&source.id)?,
        video_track_id: track_id(&video_track.id)?,
        audio_track_id: track_id(&audio_track.id)?,
        kind,
    })
}

pub(crate) fn snapshots(
    project: &Project,
    scenes: &[Scene],
    runtime: &[live_production_media::protocol::SourceSnapshot],
) -> Vec<ApiSourceSnapshot> {
    project
        .sources
        .iter()
        .map(|source| {
            let mut snapshot = crate::snapshot::source_snapshot(source, scenes);
            if let Some(status) = runtime
                .iter()
                .find(|status| status.id.as_str() == source.id.0)
            {
                for track in &mut snapshot.tracks {
                    apply_track_status(track, status);
                }
            }
            snapshot
        })
        .collect()
}

fn apply_track_status(
    track: &mut TrackSnapshot,
    status: &live_production_media::protocol::SourceSnapshot,
) {
    track.drift = match track.kind {
        TrackKind::Video => status.video_drift.clone(),
        TrackKind::Audio => status.audio_drift.clone(),
    };
    track.connection = if status.connected {
        "connected"
    } else {
        "connecting"
    };
    let buffered_ns = match track.kind {
        TrackKind::Video => {
            track.format = "video/x-raw 1280x720 30 fps";
            status.video_buffered_ns.value()
        }
        TrackKind::Audio => {
            track.format = "audio/x-raw S16LE 48 kHz stereo";
            status.audio_buffered_ns.value()
        }
    };
    track.buffer_ms = u32::try_from(buffered_ns / 1_000_000).unwrap_or(u32::MAX);
}

fn unique_adapter(project: &Project, adapter: &SourceAdapter) -> Result<SourceAdapter> {
    match adapter {
        SourceAdapter::Synthetic {
            pattern,
            frequency_hz,
        } => Ok(SourceAdapter::Synthetic {
            pattern: pattern.clone(),
            frequency_hz: *frequency_hz,
        }),
        SourceAdapter::Srt {
            bind_address,
            port,
            buffer_ms,
        } => {
            let mut candidate = *port;
            for _ in 0..=u16::MAX {
                let occupied = project.sources.iter().any(|source| {
                    matches!(source.adapter, SourceAdapter::Srt { port, .. } if port == candidate)
                });
                if !occupied {
                    return Ok(SourceAdapter::Srt {
                        bind_address: bind_address.clone(),
                        port: candidate,
                        buffer_ms: *buffer_ms,
                    });
                }
                candidate = candidate.checked_add(1).ok_or_else(|| {
                    AppError::Unavailable("No SRT listener port remains available.".into())
                })?;
            }
            Err(AppError::Unavailable(
                "No SRT listener port remains available.".into(),
            ))
        }
    }
}

fn synthetic_pattern(value: &str) -> Result<SyntheticPattern> {
    match value {
        "ball" => Ok(SyntheticPattern::Ball),
        "black" => Ok(SyntheticPattern::Black),
        "smpte" => Ok(SyntheticPattern::Smpte),
        "snow" => Ok(SyntheticPattern::Snow),
        _ => Err(AppError::Invalid(
            "Synthetic pattern must be ball, black, smpte, or snow.".into(),
        )),
    }
}

fn source_id(value: &Id) -> Result<SourceId> {
    SourceId::new(&value.0).map_err(|error| AppError::Invalid(error.to_string()))
}

fn track_id(value: &Id) -> Result<TrackId> {
    TrackId::new(&value.0).map_err(|error| AppError::Invalid(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_stable_independent_track_ids_and_scoped_srt_credential() -> Result<()> {
        // Given
        let project = Project::empty();
        let adapter = SourceAdapter::Srt {
            bind_address: "0.0.0.0".into(),
            port: 9_000,
            buffer_ms: 120,
        };

        // When
        let source = create_source(&project, "Remote camera", &adapter)?;
        let credential = format!("lp{}", uuid::Uuid::new_v4().simple());
        let spec = source_spec(&source, &credential)?;

        // Then
        assert_ne!(source.tracks[0].id, source.tracks[1].id);
        assert_eq!(spec.id.as_str(), source.id.0);
        let SourceKind::SrtListener { passphrase, .. } = spec.kind else {
            return Err(AppError::Internal(anyhow::anyhow!("expected SRT")));
        };
        assert_eq!(passphrase, credential);
        assert!(passphrase.len() >= 10);
        Ok(())
    }
}
