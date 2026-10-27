use std::{
    net::UdpSocket,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use gst::prelude::*;
use gstreamer as gst;

use super::PASSPHRASE;

pub struct PipelineGuard(gst::Pipeline);

impl PipelineGuard {
    pub fn start(&self) -> anyhow::Result<()> {
        self.0.set_state(gst::State::Playing)?;
        Ok(())
    }

    pub fn stop(&self) -> anyhow::Result<()> {
        self.0.set_state(gst::State::Null)?;
        Ok(())
    }
}

impl Drop for PipelineGuard {
    fn drop(&mut self) {
        let _ = self.0.set_state(gst::State::Null);
    }
}

pub fn sender(port: u16, timestamp_ppm: i64, wall_rate_ppm: i64) -> anyhow::Result<PipelineGuard> {
    let description = format!(
        "mpegtsmux name=mux ! srtsink uri=\"srt://127.0.0.1:{port}?mode=caller&passphrase={PASSPHRASE}&pbkeylen=16\" wait-for-connection=false auto-reconnect=false sync=false \
         videotestsrc is-live=true pattern=ball ! video/x-raw,width=640,height=360,framerate=30/1 ! x264enc tune=zerolatency key-int-max=30 ! h264parse config-interval=-1 ! identity name=video_rate ! mux. \
         audiotestsrc is-live=true wave=ticks ! audio/x-raw,rate=48000,channels=2 ! voaacenc ! aacparse ! identity name=audio_rate ! mux."
    );
    let pipeline = gst::parse::launch(&description)?
        .downcast::<gst::Pipeline>()
        .map_err(|_| anyhow::anyhow!("sender is not a pipeline"))?;
    install_timestamp_rate_probe(&pipeline, "video_rate", timestamp_ppm)
        .map_err(anyhow::Error::msg)?;
    install_timestamp_rate_probe(&pipeline, "audio_rate", timestamp_ppm)
        .map_err(anyhow::Error::msg)?;
    install_wall_rate_probe(&pipeline, "video_rate", wall_rate_ppm).map_err(anyhow::Error::msg)?;
    install_wall_rate_probe(&pipeline, "audio_rate", wall_rate_ppm).map_err(anyhow::Error::msg)?;
    Ok(PipelineGuard(pipeline))
}

pub fn rss_kib() -> anyhow::Result<u64> {
    let status = std::fs::read_to_string("/proc/self/status")?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|value| value.split_whitespace().next())
        .ok_or_else(|| anyhow::anyhow!("VmRSS unavailable"))?
        .parse()
        .map_err(Into::into)
}

pub fn free_udp_port() -> anyhow::Result<u16> {
    let socket = UdpSocket::bind(("127.0.0.1", 0))?;
    Ok(socket.local_addr()?.port())
}

fn install_timestamp_rate_probe(
    pipeline: &gst::Pipeline,
    name: &str,
    ppm: i64,
) -> Result<(), String> {
    let pad = pipeline
        .by_name(name)
        .and_then(|element| element.static_pad("src"))
        .ok_or_else(|| format!("rate probe '{name}' missing"))?;
    let first = Arc::new(AtomicU64::new(u64::MAX));
    pad.add_probe(gst::PadProbeType::BUFFER, move |_, info| {
        if let Some(buffer) = info.buffer_mut()
            && let Some(pts) = buffer.pts()
        {
            let origin = match first.compare_exchange(
                u64::MAX,
                pts.nseconds(),
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => pts.nseconds(),
                Err(value) => value,
            };
            let delta = pts.nseconds().saturating_sub(origin);
            let factor = i128::from(1_000_000_i64.saturating_add(ppm));
            let scaled = i128::from(delta).saturating_mul(factor) / 1_000_000;
            let scaled = u64::try_from(scaled).map_or(u64::MAX, std::convert::identity);
            buffer
                .make_mut()
                .set_pts(Some(gst::ClockTime::from_nseconds(
                    origin.saturating_add(scaled),
                )));
        }
        gst::PadProbeReturn::Ok
    })
    .ok_or_else(|| format!("cannot install rate probe '{name}'"))?;
    Ok(())
}

fn install_wall_rate_probe(pipeline: &gst::Pipeline, name: &str, ppm: i64) -> Result<(), String> {
    if ppm == 0 {
        return Ok(());
    }
    let factor = 1_000_000_i64
        .checked_add(ppm)
        .filter(|value| *value > 0)
        .ok_or_else(|| "wall rate factor must be positive".to_owned())?;
    let pad = pipeline
        .by_name(name)
        .and_then(|element| element.static_pad("src"))
        .ok_or_else(|| format!("wall rate probe '{name}' missing"))?;
    let origin = Arc::new(OnceLock::<(Instant, u64)>::new());
    pad.add_probe(gst::PadProbeType::BUFFER, move |_, info| {
        if let Some(pts) = info.buffer().and_then(|buffer| buffer.pts()) {
            let (started, first_pts) = origin.get_or_init(|| (Instant::now(), pts.nseconds()));
            let media_delta = pts.nseconds().saturating_sub(*first_pts);
            let target_ns = i128::from(media_delta).saturating_mul(1_000_000) / i128::from(factor);
            if let Ok(target_ns) = u64::try_from(target_ns) {
                let target = Duration::from_nanos(target_ns);
                let elapsed = started.elapsed();
                if target > elapsed {
                    std::thread::sleep(target - elapsed);
                }
            }
        }
        gst::PadProbeReturn::Ok
    })
    .ok_or_else(|| format!("cannot install wall rate probe '{name}'"))?;
    Ok(())
}
