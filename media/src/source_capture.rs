use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak, mpsc},
};

use gst::prelude::*;
use gstreamer as gst;

use crate::{source_stamp::SourceStamp, tap::TapChannels};

const VIDEO_QUEUE_CAPACITY: usize = 8;
const AUDIO_QUEUE_CAPACITY: usize = 64;

#[derive(Clone)]
pub struct SourceCapturePacket {
    pub buffer: gst::Buffer,
    pub caps: gst::Caps,
    pub stamp: SourceStamp,
    pub sequence: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceCaptureFault {
    Closed,
    InvalidCapture,
    Overflow,
}

#[derive(Clone)]
pub struct SourceCapture {
    video: Arc<TrackCapture>,
    audio: Arc<TrackCapture>,
}

impl SourceCapture {
    pub(crate) fn recording_origin(&self, anchor_media_pts: u64) -> u64 {
        [self.video.latest_media_pts(), self.audio.latest_media_pts()]
            .into_iter()
            .flatten()
            .fold(anchor_media_pts, u64::min)
    }

    pub(crate) fn subscribe(&self) -> SourceCaptureSubscription {
        SourceCaptureSubscription {
            video: self.video.subscribe(VIDEO_QUEUE_CAPACITY),
            audio: self.audio.subscribe(AUDIO_QUEUE_CAPACITY),
        }
    }
}

pub struct SourceCaptureSubscription {
    pub video: TrackSubscription,
    pub audio: TrackSubscription,
}

pub struct TrackSubscription {
    pub receiver: Option<mpsc::Receiver<SourceCapturePacket>>,
    pub(crate) fault: Arc<Mutex<Option<SourceCaptureFault>>>,
    control: SubscriptionControl,
}

impl TrackSubscription {
    pub fn close(&self) {
        self.control.close();
    }
}

struct SubscriptionControl {
    track: Weak<TrackCapture>,
    id: u64,
}

impl SubscriptionControl {
    fn close(&self) {
        if let Some(track) = self.track.upgrade()
            && let Ok(mut state) = track.state.lock()
        {
            state.subscribers.remove(&self.id);
        }
    }
}

impl Drop for SubscriptionControl {
    fn drop(&mut self) {
        self.close();
    }
}

#[derive(Default)]
pub struct SourceCaptureRegistry {
    tracks: Mutex<HashMap<String, Arc<TrackCapture>>>,
}

impl std::fmt::Debug for SourceCaptureRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SourceCaptureRegistry")
    }
}

impl SourceCaptureRegistry {
    #[must_use]
    pub fn capture(&self, channels: &TapChannels) -> SourceCapture {
        SourceCapture {
            video: self.track(&channels.video),
            audio: self.track(&channels.audio),
        }
    }

    pub(crate) fn install(&self, channel: &str, pad: &gst::Pad) -> Result<CaptureProbe, String> {
        let track = self.track(channel);
        let probe_track = Arc::clone(&track);
        let probe = pad
            .add_probe(gst::PadProbeType::BUFFER, move |pad, info| {
                let Some(buffer) = info.buffer() else {
                    return gst::PadProbeReturn::Ok;
                };
                capture_buffer(&probe_track, pad.current_caps(), buffer.to_owned());
                gst::PadProbeReturn::Ok
            })
            .ok_or_else(|| "cannot install source capture probe".to_owned())?;
        Ok(CaptureProbe {
            pad: pad.downgrade(),
            probe: Mutex::new(Some(probe)),
        })
    }

    pub(crate) fn close(&self, channels: &TapChannels) {
        if let Ok(tracks) = self.tracks.lock() {
            for channel in [&channels.video, &channels.audio] {
                if let Some(track) = tracks.get(channel) {
                    track.close();
                }
            }
        }
    }

    pub(crate) fn shutdown(&self) {
        if let Ok(tracks) = self.tracks.lock() {
            for track in tracks.values() {
                track.close();
            }
        }
    }

    fn track(&self, channel: &str) -> Arc<TrackCapture> {
        let Ok(mut tracks) = self.tracks.lock() else {
            return Arc::new(TrackCapture::default());
        };
        Arc::clone(
            tracks
                .entry(channel.to_owned())
                .or_insert_with(|| Arc::new(TrackCapture::default())),
        )
    }
}

pub struct CaptureProbe {
    pad: gst::glib::WeakRef<gst::Pad>,
    probe: Mutex<Option<gst::PadProbeId>>,
}

impl CaptureProbe {
    pub fn disconnect(&self) {
        let Some(pad) = self.pad.upgrade() else {
            return;
        };
        if let Ok(mut probe) = self.probe.lock()
            && let Some(probe) = probe.take()
        {
            pad.remove_probe(probe);
        }
    }
}

#[derive(Default)]
struct TrackCapture {
    state: Mutex<TrackCaptureState>,
}

#[derive(Default)]
struct TrackCaptureState {
    next_subscriber_id: u64,
    next_sequence: u64,
    latest_media_pts: Option<u64>,
    subscribers: HashMap<u64, Subscriber>,
}

struct Subscriber {
    sender: mpsc::SyncSender<SourceCapturePacket>,
    fault: Arc<Mutex<Option<SourceCaptureFault>>>,
}

impl TrackCapture {
    fn latest_media_pts(&self) -> Option<u64> {
        self.state.lock().ok()?.latest_media_pts
    }

    fn close(&self) {
        self.fail(SourceCaptureFault::Closed);
    }

    fn fail(&self, failure: SourceCaptureFault) {
        if let Ok(mut state) = self.state.lock() {
            for subscriber in state.subscribers.values() {
                if let Ok(mut fault) = subscriber.fault.lock() {
                    *fault = Some(failure);
                }
            }
            state.subscribers.clear();
        }
    }

    fn subscribe(self: &Arc<Self>, capacity: usize) -> TrackSubscription {
        let (sender, receiver) = mpsc::sync_channel(capacity);
        let fault = Arc::new(Mutex::new(None));
        let id = self.state.lock().map_or(u64::MAX, |mut state| {
            let id = state.next_subscriber_id;
            state.next_subscriber_id = state.next_subscriber_id.saturating_add(1);
            state.subscribers.insert(
                id,
                Subscriber {
                    sender,
                    fault: Arc::clone(&fault),
                },
            );
            id
        });
        TrackSubscription {
            receiver: Some(receiver),
            fault,
            control: SubscriptionControl {
                track: Arc::downgrade(self),
                id,
            },
        }
    }

    fn publish(&self, buffer: gst::Buffer, caps: gst::Caps, stamp: SourceStamp) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let sequence = state.next_sequence;
        state.next_sequence = state.next_sequence.saturating_add(1);
        state.latest_media_pts = Some(stamp.media_pts_ns);
        let packet = SourceCapturePacket {
            buffer,
            caps,
            stamp,
            sequence,
        };
        state.subscribers.retain(|_, subscriber| {
            match subscriber.sender.try_send(packet.clone()) {
                Ok(()) => true,
                Err(mpsc::TrySendError::Disconnected(_)) => false,
                Err(mpsc::TrySendError::Full(_)) => {
                    if let Ok(mut fault) = subscriber.fault.lock() {
                        *fault = Some(SourceCaptureFault::Overflow);
                    }
                    false
                }
            }
        });
    }
}

fn capture_buffer(track: &TrackCapture, caps: Option<gst::Caps>, buffer: gst::Buffer) {
    let Some(caps) = caps else {
        track.fail(SourceCaptureFault::InvalidCapture);
        return;
    };
    let Some(stamp) = SourceStamp::from_buffer(&buffer) else {
        track.fail(SourceCaptureFault::InvalidCapture);
        return;
    };
    track.publish(buffer, caps, stamp);
}

#[cfg(test)]
#[path = "source_capture_tests.rs"]
mod tests;
