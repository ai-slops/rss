use std::{collections::HashMap, net::TcpListener, sync::Mutex};

use gstreamer::{self as gst, prelude::*};
use thiserror::Error;

use crate::{
    protocol::{EngineRunId, TrackId},
    tap::TapChannels,
};

pub const SOURCE_AUDIO_SLOTS: usize = 8;

pub struct PreviewOutputs {
    engine_run_id: EngineRunId,
    preview_port: u16,
    program_port: u16,
    pipelines: Vec<gst::Pipeline>,
    source_pipelines: Mutex<HashMap<TrackId, SourceMonitor>>,
}

struct SourceMonitor {
    port: u16,
    pipeline: gst::Pipeline,
    clients: usize,
}

impl PreviewOutputs {
    /// Starts localhost-only WebRTC producers fed by native inter-pipeline taps.
    ///
    /// # Errors
    /// Returns an error when `GStreamer` cannot construct or start a producer.
    pub fn spawn(engine_run_id: &EngineRunId) -> Result<Self, PreviewError> {
        gst::init().map_err(|error| PreviewError::Gstreamer(error.to_string()))?;
        let preview_port = allocate_local_port()?;
        let preview = start_pipeline(&video_pipeline(engine_run_id, "preview", preview_port))?;
        let program_port = allocate_local_port()?;
        let pipelines = vec![
            preview,
            start_pipeline(&program_pipeline(engine_run_id, program_port))?,
        ];

        Ok(Self {
            engine_run_id: engine_run_id.clone(),
            preview_port,
            program_port,
            pipelines,
            source_pipelines: Mutex::new(HashMap::with_capacity(SOURCE_AUDIO_SLOTS)),
        })
    }

    #[must_use]
    pub const fn preview_port(&self) -> u16 {
        self.preview_port
    }

    #[must_use]
    pub const fn program_port(&self) -> u16 {
        self.program_port
    }

    /// Returns a bounded private source-monitor producer for the requested track.
    ///
    /// # Errors
    /// Returns an error when the source-monitor budget is full or its pipeline cannot start.
    pub fn source_audio_port(&self, track_id: &str) -> Result<u16, PreviewError> {
        let track_id = TrackId::new(track_id)
            .map_err(|error| PreviewError::InvalidTrack(error.to_string()))?;
        let mut sources = self
            .source_pipelines
            .lock()
            .map_err(|_| PreviewError::Gstreamer("source monitor state is unavailable".into()))?;
        if let Some(monitor) = sources.get_mut(&track_id) {
            monitor.clients += 1;
            return Ok(monitor.port);
        }
        if sources.len() >= SOURCE_AUDIO_SLOTS {
            return Err(PreviewError::SourceBudgetFull);
        }
        let port = allocate_local_port()?;
        let pipeline = start_pipeline(&audio_pipeline(&self.engine_run_id, &track_id, port))?;
        sources.insert(
            track_id,
            SourceMonitor {
                port,
                pipeline,
                clients: 1,
            },
        );
        drop(sources);
        Ok(port)
    }

    pub fn release_source_audio(&self, track: &str) {
        let Ok(track) = TrackId::new(track) else {
            return;
        };
        let Ok(mut sources) = self.source_pipelines.lock() else {
            return;
        };
        let remove = if let Some(monitor) = sources.get_mut(&track) {
            monitor.clients = monitor.clients.saturating_sub(1);
            monitor.clients == 0
        } else {
            false
        };
        let retired = if remove { sources.remove(&track) } else { None };
        drop(sources);
        if let Some(monitor) = retired {
            let _ = monitor.pipeline.set_state(gst::State::Null);
        }
    }
}

impl Drop for PreviewOutputs {
    fn drop(&mut self) {
        for pipeline in &self.pipelines {
            let _ = pipeline.set_state(gst::State::Null);
        }
        if let Ok(sources) = self.source_pipelines.lock() {
            for monitor in sources.values() {
                let _ = monitor.pipeline.set_state(gst::State::Null);
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum PreviewError {
    #[error("WebRTC preview setup failed: {0}")]
    Gstreamer(String),
    #[error("The eight-track private source-monitor budget is full")]
    SourceBudgetFull,
    #[error("Invalid source-monitor track: {0}")]
    InvalidTrack(String),
    #[error("Cannot allocate a localhost signaling port: {0}")]
    SignallingPort(#[from] std::io::Error),
}

fn allocate_local_port() -> Result<u16, std::io::Error> {
    TcpListener::bind(("127.0.0.1", 0))
        .and_then(|listener| listener.local_addr().map(|address| address.port()))
}

fn start_pipeline(description: &str) -> Result<gst::Pipeline, PreviewError> {
    let element = gst::parse::launch(description)
        .map_err(|error| PreviewError::Gstreamer(error.to_string()))?;
    let pipeline = element
        .downcast::<gst::Pipeline>()
        .map_err(|_| PreviewError::Gstreamer("preview description is not a pipeline".into()))?;
    for element in pipeline.children() {
        if element
            .factory()
            .is_some_and(|factory| factory.name() == "webrtcsink")
        {
            element.set_property(
                "stun-server",
                std::env::var("LIVE_PRODUCTION_STUN_SERVER").ok(),
            );
        }
    }
    pipeline
        .set_state(gst::State::Playing)
        .map_err(|error| PreviewError::Gstreamer(error.to_string()))?;
    Ok(pipeline)
}

fn video_pipeline(engine_run_id: &EngineRunId, name: &str, port: u16) -> String {
    let channel = TapChannels::preview(engine_run_id);
    format!(
        "intervideosrc channel={channel} timeout=500000000 ! queue max-size-buffers=1 leaky=downstream ! \
         videoconvert ! video/x-raw,format=I420,width=960,height=540,framerate=30/1 ! \
         webrtcsink name={name}_webrtc run-signalling-server=true signalling-server-host=127.0.0.1 \
         signalling-server-port={port} congestion-control=disabled do-fec=false \
         video-caps=video/x-vp8 meta=meta,name={name}"
    )
}

fn program_pipeline(engine_run_id: &EngineRunId, port: u16) -> String {
    let channels = TapChannels::program(engine_run_id);
    format!(
        "webrtcsink name=program_webrtc run-signalling-server=true signalling-server-host=127.0.0.1 \
         signalling-server-port={port} video-caps=video/x-vp8 audio-caps=audio/x-opus \
         congestion-control=disabled do-fec=false meta=meta,name=program \
         intervideosrc channel={} timeout=500000000 ! queue max-size-buffers=1 leaky=downstream ! \
         videoconvert ! video/x-raw,format=I420,width=960,height=540,framerate=30/1 ! program_webrtc.video_0 \
         interaudiosrc channel={} ! queue max-size-time=200000000 leaky=downstream ! \
         audioconvert ! audioresample ! audio/x-raw,rate=48000,channels=2 ! program_webrtc.audio_0",
        channels.video, channels.audio
    )
}

fn audio_pipeline(engine_run_id: &EngineRunId, track_id: &TrackId, port: u16) -> String {
    let channel = TapChannels::source_audio(engine_run_id, track_id);
    let name = format!("source-{}", track_id.as_str());
    format!(
        "interaudiosrc channel={channel} ! queue max-size-time=200000000 leaky=downstream ! \
         audioconvert ! audioresample ! audio/x-raw,rate=48000,channels=2 ! \
         webrtcsink name={name}_webrtc run-signalling-server=true signalling-server-host=127.0.0.1 \
         signalling-server-port={port} congestion-control=disabled do-fec=false \
         audio-caps=audio/x-opus meta=meta,name={name}"
    )
}

#[cfg(test)]
mod tests {
    use super::{
        EngineRunId, TrackId, allocate_local_port, audio_pipeline, program_pipeline, video_pipeline,
    };

    #[test]
    fn preview_descriptions_use_run_scoped_native_taps()
    -> Result<(), crate::protocol::IdentifierError> {
        // Given
        let engine_run_id = EngineRunId::new("engine-run-a")?;
        let audio_track_id = TrackId::new("camera-audio")?;

        // When
        let preview = video_pipeline(&engine_run_id, "preview", 18_443);
        let program = program_pipeline(&engine_run_id, 18_444);
        let source = audio_pipeline(&engine_run_id, &audio_track_id, 18_500);

        // Then
        assert!(preview.contains("channel=live-engine-run-a-preview-video-v1"));
        assert!(program.contains("channel=live-engine-run-a-program-video-v1"));
        assert!(program.contains("channel=live-engine-run-a-program-audio-v1"));
        assert!(source.contains("channel=live-engine-run-a-source-audio-v1-camera-audio"));
        Ok(())
    }

    #[test]
    fn signalling_ports_are_allocated_per_server_instance() -> Result<(), std::io::Error> {
        // Given / When
        let preview_port = allocate_local_port()?;
        let _reservation = std::net::TcpListener::bind(("127.0.0.1", preview_port))?;
        let program_port = allocate_local_port()?;

        // Then
        assert_ne!(preview_port, program_port);
        Ok(())
    }
}
