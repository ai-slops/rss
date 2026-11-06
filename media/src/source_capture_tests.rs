use super::*;
use crate::{recording_provenance::RecordingTrackKind, source_stamp::SourceStamp};

impl SourceCaptureRegistry {
    pub(crate) fn publish_for_test(
        &self,
        channel: &str,
        buffer: gst::Buffer,
        caps: gst::Caps,
        stamp: SourceStamp,
    ) {
        self.track(channel).publish(buffer, caps, stamp);
    }

    pub(crate) fn subscriber_count(&self, channels: &TapChannels) -> usize {
        [&channels.video, &channels.audio]
            .into_iter()
            .map(|channel| self.track(channel).subscriber_count())
            .sum()
    }
}

impl TrackCapture {
    fn subscriber_count(&self) -> usize {
        self.state.lock().map_or(0, |state| state.subscribers.len())
    }
}

fn packet(generation: u64) -> (gst::Buffer, gst::Caps, SourceStamp) {
    (
        gst::Buffer::new(),
        gst::Caps::builder("video/x-raw").build(),
        SourceStamp {
            engine_run_id: "run".to_owned(),
            source_id: "source".to_owned(),
            track_id: "track".to_owned(),
            track_kind: RecordingTrackKind::Video,
            continuity_generation: generation,
            media_pts_ns: generation,
            duration_ns: 1,
        },
    )
}

#[test]
fn full_subscriber_queue_sets_sticky_fault_and_removes_only_subscriber() -> anyhow::Result<()> {
    gst::init()?;
    let track = Arc::new(TrackCapture::default());
    let subscription = track.subscribe(1);
    let (buffer, caps, stamp) = packet(1);
    track.publish(buffer, caps, stamp);
    let (buffer, caps, stamp) = packet(2);
    track.publish(buffer, caps, stamp);

    assert_eq!(
        *subscription.fault.lock().expect("fault"),
        Some(SourceCaptureFault::Overflow)
    );
    assert!(subscription.receiver.expect("receiver").recv().is_ok());
    Ok(())
}

#[test]
fn closing_subscription_disconnects_blocked_receiver() -> anyhow::Result<()> {
    gst::init()?;
    let track = Arc::new(TrackCapture::default());
    let mut subscription = track.subscribe(1);
    subscription.close();
    assert!(
        subscription
            .receiver
            .take()
            .expect("receiver")
            .recv_timeout(std::time::Duration::from_millis(50))
            .is_err()
    );
    Ok(())
}

#[test]
fn closing_track_sets_explicit_fault_and_wakes_receiver() -> anyhow::Result<()> {
    gst::init()?;
    let track = Arc::new(TrackCapture::default());
    let mut subscription = track.subscribe(1);
    track.close();
    assert_eq!(
        *subscription.fault.lock().expect("fault"),
        Some(SourceCaptureFault::Closed)
    );
    assert!(
        subscription
            .receiver
            .take()
            .expect("receiver")
            .recv_timeout(std::time::Duration::from_millis(50))
            .is_err()
    );
    Ok(())
}

#[test]
fn missing_stamp_faults_subscriber_instead_of_extending_previous_interval() -> anyhow::Result<()> {
    gst::init()?;
    let track = Arc::new(TrackCapture::default());
    let mut subscription = track.subscribe(2);
    let (buffer, caps, stamp) = packet(1);
    track.publish(buffer, caps.clone(), stamp);
    capture_buffer(&track, Some(caps), gst::Buffer::new());

    assert!(
        subscription
            .receiver
            .as_ref()
            .expect("receiver")
            .recv()
            .is_ok()
    );
    assert_eq!(
        *subscription.fault.lock().expect("fault"),
        Some(SourceCaptureFault::InvalidCapture)
    );
    assert!(
        subscription
            .receiver
            .take()
            .expect("receiver")
            .recv_timeout(std::time::Duration::from_millis(50))
            .is_err()
    );
    Ok(())
}

#[test]
fn missing_caps_faults_subscriber() -> anyhow::Result<()> {
    gst::init()?;
    let track = Arc::new(TrackCapture::default());
    let subscription = track.subscribe(1);
    capture_buffer(&track, None, gst::Buffer::new());
    assert_eq!(
        *subscription.fault.lock().expect("fault"),
        Some(SourceCaptureFault::InvalidCapture)
    );
    Ok(())
}

#[test]
fn latest_authoritative_pts_is_retained_without_subscribers_for_common_origin() -> anyhow::Result<()>
{
    gst::init()?;
    let registry = SourceCaptureRegistry::default();
    let channels = TapChannels {
        video: "origin-video".to_owned(),
        audio: "origin-audio".to_owned(),
    };
    let capture = registry.capture(&channels);
    let (video_buffer, video_caps, mut video_stamp) = packet(1);
    video_stamp.media_pts_ns = 80;
    registry.publish_for_test(&channels.video, video_buffer, video_caps, video_stamp);
    let (audio_buffer, audio_caps, mut audio_stamp) = packet(1);
    audio_stamp.media_pts_ns = 90;
    registry.publish_for_test(&channels.audio, audio_buffer, audio_caps, audio_stamp);
    assert_eq!(capture.recording_origin(100), 80);
    Ok(())
}
