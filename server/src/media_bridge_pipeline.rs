use live_production_media::engine::EngineConfig;

use crate::error::Result;

pub(crate) fn engine_config() -> Result<EngineConfig> {
    Ok(EngineConfig::new(&pipeline_description(), []))
}

fn pipeline_description() -> String {
    "compositor name=preview_compositor background=black force-live=true ignore-inactive-pads=true ! \
     video/x-raw,format=I420,width=960,height=540,framerate=30/1 ! tee name=preview_video \
     preview_video. ! queue max-size-buffers=1 leaky=downstream ! intervideosink name=preview_video_tap channel=live-{engine_run_id}-preview-video-v1 sync=false async=false \
     preview_video. ! queue max-size-buffers=1 leaky=downstream ! fakesink sync=false \
     compositor name=program_compositor background=black force-live=true ignore-inactive-pads=true ! \
     video/x-raw,format=I420,width=960,height=540,framerate=30/1 ! tee name=program_video \
     program_video. ! queue max-size-buffers=2 leaky=downstream ! intervideosink name=program_video_tap channel=live-{engine_run_id}-program-video-v1 sync=false async=false \
     program_video. ! queue max-size-buffers=2 leaky=downstream ! fakesink sync=false \
     audiomixer name=program_mixer force-live=true ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! tee name=program_audio \
     program_audio. ! queue max-size-time=200000000 leaky=downstream ! interaudiosink name=program_audio_tap channel=live-{engine_run_id}-program-audio-v1 sync=false async=false \
     program_audio. ! queue max-size-time=200000000 leaky=downstream ! fakesink sync=false"
        .to_owned()
}
