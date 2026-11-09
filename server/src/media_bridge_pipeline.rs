use live_production_media::engine::EngineConfig;

use crate::{
    error::{AppError, Result},
    model::{Canvas, Project},
};

pub(crate) fn engine_config() -> Result<EngineConfig> {
    Ok(EngineConfig::new(&pipeline_description()?, []))
}

fn pipeline_description() -> Result<String> {
    let canvas = canvas_config()?;
    Ok("compositor name=preview_compositor background=black force-live=true ignore-inactive-pads=true ! \
     video/x-raw,format=I420,width=960,height=540,framerate=30/1 ! tee name=preview_video \
     preview_video. ! queue max-size-buffers=1 leaky=downstream ! intervideosink name=preview_video_tap channel=live-{engine_run_id}-preview-video-v1 sync=false async=false \
     preview_video. ! queue max-size-buffers=1 leaky=downstream ! fakesink sync=false \
     compositor name=program_compositor background=black force-live=true ignore-inactive-pads=true ! \
     video/x-raw,format=I420,width=960,height=540,framerate=30/1 ! tee name=program_video \
     program_video. ! queue max-size-buffers=2 leaky=downstream ! intervideosink name=program_video_tap channel=live-{engine_run_id}-program-video-v1 sync=false async=false \
     program_video. ! queue max-size-buffers=2 leaky=downstream ! fakesink sync=false \
     audiomixer name=program_mixer force-live=true ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! tee name=program_audio \
     program_audio. ! queue max-size-time=200000000 leaky=downstream ! interaudiosink name=program_audio_tap channel=live-{engine_run_id}-program-audio-v1 sync=false async=false \
     program_audio. ! queue max-size-time=200000000 leaky=downstream ! interaudiosink channel=live-{engine_run_id}-program-monitor-audio-v1 sync=false async=false \
     program_audio. ! queue max-size-time=200000000 leaky=downstream ! interaudiosink channel=live-{engine_run_id}-program-recording-audio-v1 sync=false async=false \
     program_audio. ! queue max-size-time=200000000 leaky=downstream ! fakesink sync=false"
        .replace("width=960,height=540,framerate=30/1", &format!("width={},height={},framerate={}/1", canvas.width, canvas.height, canvas.fps_numerator)))
}

fn canvas_config() -> Result<Canvas> {
    fn setting(name: &str, default: u32, minimum: u32, maximum: u32) -> Result<u32> {
        let value = std::env::var(name).map_or(Ok(default), |value| {
            value
                .parse::<u32>()
                .map_err(|_| AppError::Invalid(format!("Invalid {name}")))
        })?;
        if !(minimum..=maximum).contains(&value) {
            return Err(AppError::Invalid(format!("Invalid {name}")));
        }
        Ok(value)
    }
    Ok(Canvas {
        width: setting("LIVE_PRODUCTION_VIDEO_WIDTH", 960, 64, 3840)?,
        height: setting("LIVE_PRODUCTION_VIDEO_HEIGHT", 540, 64, 2160)?,
        fps_numerator: setting("LIVE_PRODUCTION_VIDEO_FPS", 30, 1, 60)?,
        fps_denominator: 1,
    })
}

pub(crate) fn apply_canvas_profile(project: &mut Project) -> Result<bool> {
    if ![
        "LIVE_PRODUCTION_VIDEO_WIDTH",
        "LIVE_PRODUCTION_VIDEO_HEIGHT",
        "LIVE_PRODUCTION_VIDEO_FPS",
    ]
    .iter()
    .any(|name| std::env::var_os(name).is_some())
    {
        return Ok(false);
    }
    let canvas = canvas_config()?;
    if (
        project.canvas.width,
        project.canvas.height,
        project.canvas.fps_numerator,
        project.canvas.fps_denominator,
    ) == (canvas.width, canvas.height, canvas.fps_numerator, 1)
    {
        return Ok(false);
    }
    let scale_x = f64::from(canvas.width) / f64::from(project.canvas.width);
    let scale_y = f64::from(canvas.height) / f64::from(project.canvas.height);
    for scene in &mut project.scenes {
        for item in &mut scene.items {
            item.rect.x *= scale_x;
            item.rect.y *= scale_y;
            item.rect.width *= scale_x;
            item.rect.height *= scale_y;
        }
        scene.revision = crate::validation::next_revision(&scene.revision)?;
    }
    project.canvas = canvas;
    Ok(true)
}
