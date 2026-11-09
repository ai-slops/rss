use crate::{
    protocol::{EngineRunId, SourceKind, SourceSpec, SyntheticPattern},
    tap::TapChannels,
};

const MAXIMUM_HISTORY_NS: u64 = 4_000_000_000;

pub struct SourceNames {
    pub video_history: String,
    pub audio_history: String,
    pub video_ingest: String,
    pub audio_ingest: String,
    pub video_timing: String,
    pub audio_timing: String,
    pub video_playout: String,
    pub audio_playout: String,
    pub video_tee: String,
    pub audio_tee: String,
    pub srt_listener: String,
}

impl SourceNames {
    pub fn new(stem: &str) -> Self {
        Self {
            video_history: format!("source_{stem}_video_history"),
            audio_history: format!("source_{stem}_audio_history"),
            video_ingest: format!("source_{stem}_video_ingest"),
            audio_ingest: format!("source_{stem}_audio_ingest"),
            video_timing: format!("source_{stem}_video_timing"),
            audio_timing: format!("source_{stem}_audio_timing"),
            video_playout: format!("source_{stem}_video_playout"),
            audio_playout: format!("source_{stem}_audio_playout"),
            video_tee: format!("source_{stem}_video_tee"),
            audio_tee: format!("source_{stem}_audio_tee"),
            srt_listener: format!("source_{stem}_srt_listener"),
        }
    }
}

pub fn source_description(
    spec: &SourceSpec,
    names: &SourceNames,
    engine_run_id: &EngineRunId,
) -> String {
    let clean = TapChannels::clean(engine_run_id, &spec.video_track_id, &spec.audio_track_id);
    let source = TapChannels::source(engine_run_id, &spec.video_track_id, &spec.audio_track_id);
    let (width, height, _) = source_video_format();
    let video_tail = format!(
        "videoconvert ! videoscale ! video/x-raw,format=I420,width={width},height={height} ! identity name={} ! tee name=raw_video raw_video. ! queue max-size-buffers=2 leaky=downstream ! intervideosink channel={} sync=false async=false raw_video. ! queue name={} max-size-bytes=0 max-size-buffers=0 max-size-time={MAXIMUM_HISTORY_NS} ! identity name={} ! clocksync name={} sync=true ! tee name={} {}. ! queue max-size-buffers=2 leaky=downstream ! intervideosink channel={} sync=false async=false",
        names.video_ingest,
        quote(&clean.video),
        names.video_history,
        names.video_timing,
        names.video_playout,
        names.video_tee,
        names.video_tee,
        quote(&source.video),
    );
    let audio_tail = format!(
        "audioconvert ! audioresample ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! identity name={} ! tee name=raw_audio raw_audio. ! queue max-size-buffers=8 leaky=downstream ! interaudiosink channel={} sync=false async=false raw_audio. ! queue name={} max-size-bytes=0 max-size-buffers=0 max-size-time={MAXIMUM_HISTORY_NS} ! identity name={} ! clocksync name={} sync=true ! tee name={} {}. ! queue max-size-buffers=8 leaky=downstream ! interaudiosink channel={} sync=false async=false {}. ! queue max-size-buffers=8 leaky=downstream ! interaudiosink channel={} sync=false async=false",
        names.audio_ingest,
        quote(&clean.audio),
        names.audio_history,
        names.audio_timing,
        names.audio_playout,
        names.audio_tee,
        names.audio_tee,
        quote(&source.audio),
        names.audio_tee,
        quote(&TapChannels::source_monitor_audio(
            engine_run_id,
            &spec.audio_track_id
        )),
    );
    match &spec.kind {
        SourceKind::Synthetic {
            pattern,
            frequency_hz,
        } => format!(
            "videotestsrc is-live=true pattern={} ! {video_tail} audiotestsrc is-live=true wave=sine freq={frequency_hz} ! {audio_tail}",
            pattern_name(*pattern)
        ),
        SourceKind::SrtListener {
            bind_address,
            port,
            latency,
            passphrase,
        } => {
            let uri = format!(
                "srt://{bind_address}:{port}?mode=listener&latency={}&passphrase={passphrase}&pbkeylen=16",
                latency.value()
            );
            format!(
                "srtsrc name={} uri={} authentication=true wait-for-connection=false keep-listening=true automatic-eos=false auto-reconnect=true ! tsparse ! tsdemux name=demux demux. ! queue ! h264parse ! avdec_h264 ! {video_tail} demux. ! queue ! aacparse ! avdec_aac ! {audio_tail}",
                names.srt_listener,
                quote(&uri)
            )
        }
    }
}

pub fn validate_source(spec: &SourceSpec) -> Result<(), String> {
    if spec.video_track_id == spec.audio_track_id {
        return Err("source video and audio tracks must have distinct IDs".to_owned());
    }
    match &spec.kind {
        SourceKind::Synthetic { frequency_hz, .. } if !(20..=20_000).contains(frequency_hz) => {
            Err("synthetic frequency must be between 20 and 20000 Hz".to_owned())
        }
        SourceKind::SrtListener {
            bind_address,
            latency,
            passphrase,
            ..
        } if bind_address.parse::<std::net::IpAddr>().is_err()
            || latency.value() < 0
            || !(10..=79).contains(&passphrase.len())
            || !passphrase
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')) =>
        {
            Err(
                "SRT requires non-negative latency and a 10-79 character scoped passphrase"
                    .to_owned(),
            )
        }
        SourceKind::Synthetic { .. } | SourceKind::SrtListener { .. } => Ok(()),
    }
}

const fn pattern_name(pattern: SyntheticPattern) -> &'static str {
    match pattern {
        SyntheticPattern::Ball => "ball",
        SyntheticPattern::Black => "black",
        SyntheticPattern::Smpte => "smpte",
        SyntheticPattern::Snow => "snow",
    }
}

pub fn element_stem(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() {
                char::from(byte)
            } else {
                '_'
            }
        })
        .collect()
}

pub fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Milliseconds, SourceId};

    #[test]
    fn decoded_history_queues_have_explicit_time_only_bounds() -> anyhow::Result<()> {
        let spec = SourceSpec {
            id: SourceId::new("bounded-source")?,
            video_track_id: crate::protocol::TrackId::new("bounded-video")?,
            audio_track_id: crate::protocol::TrackId::new("bounded-audio")?,
            kind: SourceKind::SrtListener {
                bind_address: "127.0.0.1".to_owned(),
                port: 9_000,
                latency: Milliseconds::new(120),
                passphrase: "boundedCredential42".to_owned(),
            },
        };
        let description = source_description(
            &spec,
            &SourceNames::new("bounded"),
            &EngineRunId::new("bounded-run")?,
        );
        assert_eq!(description.matches("max-size-bytes=0").count(), 2);
        assert_eq!(description.matches("max-size-buffers=0").count(), 2);
        assert_eq!(description.matches("max-size-time=4000000000").count(), 2);
        Ok(())
    }
}

pub fn source_video_format() -> (u32, u32, u32) {
    fn setting(name: &str, default: u32, maximum: u32) -> u32 {
        std::env::var(name)
            .ok()
            .and_then(|value| value.parse().ok())
            .filter(|value| *value > 0 && *value <= maximum)
            .unwrap_or(default)
    }
    (
        setting("LIVE_PRODUCTION_VIDEO_WIDTH", 1280, 3840),
        setting("LIVE_PRODUCTION_VIDEO_HEIGHT", 720, 2160),
        setting("LIVE_PRODUCTION_VIDEO_FPS", 30, 60),
    )
}
