use std::{
    ops::ControlFlow,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
};

use gst::buffer::BufferMetaForeachAction;
use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;

use crate::{
    recording_provenance::ProvenanceCollector,
    source_capture::{SourceCapture, SourceCaptureFault, SourceCapturePacket, TrackSubscription},
    source_feeder_limits::{app_source_at_limit, fault_message},
    source_sequence::SourceSequence,
};

pub struct SourceFeeders {
    stopped: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    controls: Vec<TrackSubscription>,
    workers: Vec<JoinHandle<()>>,
    app_sources: Vec<gst_app::AppSrc>,
}

struct FeedContext {
    app_source: gst_app::AppSrc,
    media_origin_ns: u64,
    provenance: Arc<ProvenanceCollector>,
    stopped: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
}

#[derive(Clone, Copy)]
struct StartContext<'a> {
    pipeline: &'a gst::Pipeline,
    capture: &'a SourceCapture,
    media_origin_ns: u64,
    provenance: &'a Arc<ProvenanceCollector>,
}

impl SourceFeeders {
    pub fn start(
        pipeline: &gst::Pipeline,
        capture: &SourceCapture,
        media_origin_ns: u64,
        provenance: &Arc<ProvenanceCollector>,
    ) -> Result<Self, String> {
        Self::start_with_spawner(
            StartContext {
                pipeline,
                capture,
                media_origin_ns,
                provenance,
            },
            spawn_track,
        )
    }

    fn start_with_spawner(
        start: StartContext<'_>,
        mut spawn: impl FnMut(
            &str,
            &mut TrackSubscription,
            FeedContext,
        ) -> Result<JoinHandle<()>, String>,
    ) -> Result<Self, String> {
        let subscription = start.capture.subscribe();
        let video = app_source(start.pipeline, "recording_video_input")?;
        let audio = app_source(start.pipeline, "recording_audio_input")?;
        let stopped = Arc::new(AtomicBool::new(false));
        let error = Arc::new(Mutex::new(None));
        let mut feeders = Self {
            stopped,
            error,
            controls: vec![subscription.video, subscription.audio],
            workers: Vec::with_capacity(2),
            app_sources: vec![video, audio],
        };
        let video_context = feeders.feed_context(0, start.media_origin_ns, start.provenance);
        let video_worker = spawn(
            "recording-video-feeder",
            &mut feeders.controls[0],
            video_context,
        )?;
        feeders.workers.push(video_worker);
        let audio_context = feeders.feed_context(1, start.media_origin_ns, start.provenance);
        let audio_worker = spawn(
            "recording-audio-feeder",
            &mut feeders.controls[1],
            audio_context,
        )?;
        feeders.workers.push(audio_worker);
        Ok(feeders)
    }

    pub fn error(&self) -> Option<String> {
        self.error
            .lock()
            .ok()
            .and_then(|value| value.clone())
            .or_else(|| {
                self.control_fault()
                    .map(|fault| fault_message(fault).to_owned())
            })
    }

    pub fn stop(mut self) -> Result<(), String> {
        self.shutdown();
        self.error().map_or(Ok(()), Err)
    }

    fn feed_context(
        &self,
        index: usize,
        media_origin_ns: u64,
        provenance: &Arc<ProvenanceCollector>,
    ) -> FeedContext {
        FeedContext {
            app_source: self.app_sources[index].clone(),
            media_origin_ns,
            provenance: Arc::clone(provenance),
            stopped: Arc::clone(&self.stopped),
            error: Arc::clone(&self.error),
        }
    }

    fn shutdown(&mut self) {
        if let Some(fault) = self.control_fault() {
            set_error(&self.error, fault_message(fault));
        }
        self.stopped.store(true, Ordering::Release);
        for control in &self.controls {
            control.close();
        }
        for worker in self.workers.drain(..) {
            if worker.join().is_err() {
                set_error(&self.error, "source capture feeder panicked");
            }
        }
        for source in &self.app_sources {
            let _ = source.end_of_stream();
        }
    }

    fn control_fault(&self) -> Option<SourceCaptureFault> {
        self.controls
            .iter()
            .find_map(|control| control.fault.lock().ok().and_then(|value| *value))
    }
}

impl Drop for SourceFeeders {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn spawn_track(
    name: &str,
    subscription: &mut TrackSubscription,
    context: FeedContext,
) -> Result<JoinHandle<()>, String> {
    let receiver = subscription
        .receiver
        .take()
        .ok_or_else(|| "source capture receiver already started".to_owned())?;
    let source_fault = Arc::clone(&subscription.fault);
    thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || {
            feed_track(&receiver, &source_fault, &context);
        })
        .map_err(|failure| failure.to_string())
}

fn feed_track(
    receiver: &mpsc::Receiver<SourceCapturePacket>,
    source_fault: &Mutex<Option<SourceCaptureFault>>,
    context: &FeedContext,
) {
    let mut sequence = SourceSequence::default();
    let mut current_caps: Option<gst::Caps> = None;
    while !context.stopped.load(Ordering::Acquire) {
        if let Some(fault) = source_fault.lock().ok().and_then(|value| *value) {
            set_error(&context.error, fault_message(fault));
            return;
        }
        let packet = match receiver.recv() {
            Ok(packet) => packet,
            Err(_) if context.stopped.load(Ordering::Acquire) => return,
            Err(_) => {
                set_error(&context.error, "source capture subscription closed");
                return;
            }
        };
        if let Err(message) = sequence.accept(packet.sequence, packet.stamp.continuity_generation) {
            set_error(&context.error, message);
            return;
        }
        let Some(local_pts_ns) = packet
            .stamp
            .media_pts_ns
            .checked_sub(context.media_origin_ns)
        else {
            set_error(
                &context.error,
                "source capture predates recording clock origin",
            );
            return;
        };
        if current_caps.as_ref() != Some(&packet.caps) {
            context.app_source.set_caps(Some(&packet.caps));
            current_caps = Some(packet.caps.clone());
        }
        let mut buffer = packet.buffer;
        {
            let buffer = buffer.make_mut();
            buffer.set_pts(Some(gst::ClockTime::from_nseconds(local_pts_ns)));
            buffer.set_dts(None);
            buffer.set_duration(Some(gst::ClockTime::from_nseconds(
                packet.stamp.duration_ns,
            )));
            buffer.foreach_meta_mut(|mut meta| {
                let action = if meta.downcast_ref::<gst::ReferenceTimestampMeta>().is_some() {
                    BufferMetaForeachAction::Remove
                } else {
                    BufferMetaForeachAction::Keep
                };
                ControlFlow::Continue(action)
            });
        }
        if app_source_at_limit(&context.app_source) {
            set_error(&context.error, "recording appsrc queue reached its bound");
            return;
        }
        if context.app_source.push_buffer(buffer).is_err() {
            set_error(&context.error, "recording appsrc rejected captured media");
            return;
        }
        context.provenance.record(&packet.stamp, local_pts_ns);
    }
}

fn app_source(pipeline: &gst::Pipeline, name: &str) -> Result<gst_app::AppSrc, String> {
    pipeline
        .by_name(name)
        .ok_or_else(|| format!("missing {name}"))?
        .downcast::<gst_app::AppSrc>()
        .map_err(|_| format!("{name} is not appsrc"))
}

fn set_error(error: &Mutex<Option<String>>, message: &str) {
    if let Ok(mut value) = error.lock()
        && value.is_none()
    {
        *value = Some(message.to_owned());
    }
}

#[cfg(test)]
#[path = "source_feeder_tests.rs"]
mod tests;
