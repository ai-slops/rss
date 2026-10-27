use live_production_media::{engine::EngineConfig, protocol::TrackId};

use crate::error::{AppError, Result};

pub(crate) const MEDIA_TRACK_SLOTS: usize = 8;

pub(crate) fn engine_config() -> Result<EngineConfig> {
    let description = pipeline_description();
    let mut config = EngineConfig::new(&description, []);
    for slot in 0..MEDIA_TRACK_SLOTS {
        let video_element = if slot == 0 {
            "video_timing_source".into()
        } else {
            format!("video_track_{slot}")
        };
        let audio_element = if slot == 0 {
            "audio_timing_source".into()
        } else {
            format!("audio_track_{slot}")
        };
        config = config
            .with_track(track_id(&video_track_id(slot))?, &video_element)
            .with_track(track_id(&audio_track_id(slot))?, &audio_element)
            .with_audio_route(track_id(&audio_track_id(slot))?, &format!("sink_{slot}"));
    }
    Ok(config)
}

#[must_use]
pub(crate) fn video_track_id(slot: usize) -> String {
    format!("video-{slot}")
}

#[must_use]
pub(crate) fn audio_track_id(slot: usize) -> String {
    format!("audio-{slot}")
}

fn track_id(value: &str) -> Result<TrackId> {
    TrackId::new(value).map_err(|error| AppError::Invalid(error.to_string()))
}

fn pipeline_description() -> String {
    let mut description = String::from(
        "compositor name=preview_compositor background=black ! videoconvert ! fakesink sync=true \
         compositor name=program_compositor background=black ! videoconvert ! fakesink sync=true \
         audiomixer name=program_mixer ! audioconvert ! audioresample ! fakesink sync=true ",
    );
    for slot in 0..MEDIA_TRACK_SLOTS {
        let video_name = if slot == 0 {
            "video_timing_source".into()
        } else {
            format!("video_track_{slot}")
        };
        let audio_name = if slot == 0 {
            "audio_timing_source".into()
        } else {
            format!("audio_track_{slot}")
        };
        description.push_str(&format!(
            "videotestsrc name={video_name} is-live=true pattern={} ! tee name=scene_slot{slot} \
             scene_slot{slot}. ! queue max-size-buffers=4 leaky=downstream ! preview_compositor.sink_{slot} \
             scene_slot{slot}. ! queue max-size-buffers=4 leaky=downstream ! program_compositor.sink_{slot} \
             audiotestsrc name={audio_name} is-live=true wave=sine freq={} ! audioconvert ! \
             audioresample ! queue max-size-time=5000000000 ! program_mixer.sink_{slot} ",
            slot % 4,
            220 + slot * 55,
        ));
    }
    description
}
