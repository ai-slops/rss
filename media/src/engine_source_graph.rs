use crate::{
    protocol::{EngineRunId, SourceKind, SourceSpec, SyntheticPattern},
    tap::TapChannels,
};

const MAXIMUM_HISTORY_NS: u64 = 5_000_000_000;

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
    let video_tail = format!(
        "videoconvert ! videoscale ! video/x-raw,width=1280,height=720,framerate=30/1 ! identity name={} ! tee name=raw_video raw_video. ! queue max-size-buffers=2 leaky=downstream ! intervideosink channel={} sync=false async=false raw_video. ! queue name={} max-size-time={MAXIMUM_HISTORY_NS} ! identity name={} ! clocksync name={} sync=true ! tee name={} {}. ! queue max-size-buffers=2 leaky=downstream ! intervideosink channel={} sync=false async=false",
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
        "audioconvert ! audioresample ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! identity name={} ! tee name=raw_audio raw_audio. ! queue max-size-buffers=8 leaky=downstream ! interaudiosink channel={} sync=false async=false raw_audio. ! queue name={} max-size-time={MAXIMUM_HISTORY_NS} ! identity name={} ! clocksync name={} sync=true ! tee name={} {}. ! queue max-size-buffers=8 leaky=downstream ! interaudiosink channel={} sync=false async=false",
        names.audio_ingest,
        quote(&clean.audio),
        names.audio_history,
        names.audio_timing,
        names.audio_playout,
        names.audio_tee,
        names.audio_tee,
        quote(&source.audio),
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
            port,
            latency,
            passphrase,
        } => {
            let uri = format!(
                "srt://:{port}?mode=listener&latency={}&passphrase={passphrase}&pbkeylen=16",
                latency.value()
            );
            format!(
                "srtsrc uri={} wait-for-connection=false ! tsparse ! tsdemux name=demux demux. ! queue ! h264parse ! avdec_h264 ! {video_tail} demux. ! queue ! aacparse ! avdec_aac ! {audio_tail}",
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
            latency,
            passphrase,
            ..
        } if latency.value() < 0
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
