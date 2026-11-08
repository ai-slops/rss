use std::{
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use gst::prelude::*;
use gstreamer as gst;

use crate::{output::MediaTap, output_pipeline::recording_pipeline};

#[test]
fn synchronous_mux_has_no_detached_finalizer_at_pipeline_eos() -> anyhow::Result<()> {
    let asynchronous = run_finalization(true)?;
    eprintln!("async observation: {asynchronous:?}");
    assert!(asynchronous.held_previous_sink);
    assert!(asynchronous.sinks_added >= 2);
    assert!(asynchronous.closed_before_release > 0);
    assert!(!asynchronous.eos_before_release);
    assert!(asynchronous.closed_after_release > 0);
    assert!(asynchronous.eos_after_release);

    let synchronous = run_finalization(false)?;
    eprintln!("sync observation: {synchronous:?}");
    assert!(!synchronous.held_previous_sink);
    assert!(synchronous.sinks_added <= 1);
    assert!(synchronous.closed_after_release > 0);
    assert!(synchronous.eos_after_release);
    Ok(())
}

#[derive(Debug)]
struct Observation {
    held_previous_sink: bool,
    sinks_added: usize,
    closed_before_release: usize,
    eos_before_release: bool,
    closed_after_release: usize,
    eos_after_release: bool,
}

struct Hold {
    release: Mutex<bool>,
    changed: Condvar,
    entered: AtomicBool,
}

fn run_finalization(async_finalize: bool) -> anyhow::Result<Observation> {
    gst::init()?;
    let id = uuid::Uuid::now_v7();
    let tap = MediaTap::new(
        &format!("ownership-video-{id}"),
        &format!("ownership-audio-{id}"),
    )?;
    let directory = tempfile::tempdir()?;
    let pattern = directory.path().join("ownership-%05d.mkv");
    let pattern = pattern
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("recording pattern"))?;
    let recorder = recording_pipeline(&tap, pattern, Duration::from_secs(1))?;
    let mux = recorder
        .by_name("recording_mux")
        .ok_or_else(|| anyhow::anyhow!("recording mux"))?;
    mux.set_property("async-finalize", async_finalize);
    let producer = gst::parse::launch(&format!(
        "videotestsrc is-live=true pattern=black ! video/x-raw,width=320,height=180,framerate=30/1 ! intervideosink channel={} sync=false async=false \
         audiotestsrc is-live=true wave=sine ! audio/x-raw,rate=48000,channels=2 ! interaudiosink channel={} sync=false async=false",
        tap.video_channel(),
        tap.audio_channel()
    ))?
    .downcast::<gst::Pipeline>()
    .map_err(|_| anyhow::anyhow!("producer pipeline"))?;
    let hold = Arc::new(Hold {
        release: Mutex::new(false),
        changed: Condvar::new(),
        entered: AtomicBool::new(false),
    });
    let sinks_added = Arc::new(AtomicUsize::new(0));
    connect_sink_hold(&mux, async_finalize, &hold, &sinks_added);
    let closed = Arc::new(AtomicUsize::new(0));
    let eos = Arc::new(AtomicBool::new(false));
    observe_bus(&recorder, &closed, &eos)?;
    producer.set_state(gst::State::Playing)?;
    recorder.set_state(gst::State::Playing)?;
    let held_previous_sink = if async_finalize {
        wait_until(Duration::from_secs(4), || {
            hold.entered.load(Ordering::Acquire) && sinks_added.load(Ordering::Acquire) >= 2
        })
    } else {
        std::thread::sleep(Duration::from_millis(2200));
        false
    };
    let sent_eos = recorder.send_event(gst::event::Eos::new());
    if async_finalize {
        std::thread::sleep(Duration::from_millis(250));
    } else {
        let _ = wait_until(Duration::from_secs(2), || eos.load(Ordering::Acquire));
    }
    let closed_before_release = closed.load(Ordering::Acquire);
    let eos_before_release = eos.load(Ordering::Acquire);
    release_hold(&hold);
    let final_message = recorder.bus().and_then(|bus| {
        bus.timed_pop_filtered(
            gst::ClockTime::from_seconds(10),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        )
    });
    let eos_after_release = eos.load(Ordering::Acquire)
        && final_message
            .as_ref()
            .is_some_and(|message| matches!(message.view(), gst::MessageView::Eos(_)));
    let closed_after_release = closed.load(Ordering::Acquire);
    let _ = recorder.set_state(gst::State::Null);
    let _ = producer.set_state(gst::State::Null);
    anyhow::ensure!(sent_eos, "pipeline rejected EOS");
    Ok(Observation {
        held_previous_sink,
        sinks_added: sinks_added.load(Ordering::Acquire),
        closed_before_release,
        eos_before_release,
        closed_after_release,
        eos_after_release,
    })
}

fn connect_sink_hold(
    mux: &gst::Element,
    enabled: bool,
    hold: &Arc<Hold>,
    sinks_added: &Arc<AtomicUsize>,
) {
    let hold = Arc::clone(hold);
    let sinks_added = Arc::clone(sinks_added);
    mux.connect("sink-added", false, move |values| {
        let index = sinks_added.fetch_add(1, Ordering::AcqRel);
        if enabled
            && index == 0
            && let Ok(sink) = values[1].get::<gst::Element>()
            && let Some(pad) = sink.static_pad("sink")
        {
            let probe_hold = Arc::clone(&hold);
            pad.add_probe(gst::PadProbeType::EVENT_DOWNSTREAM, move |_, info| {
                if info
                    .event()
                    .is_some_and(|event| matches!(event.view(), gst::EventView::Eos(_)))
                {
                    probe_hold.entered.store(true, Ordering::Release);
                    if let Ok(release) = probe_hold.release.lock() {
                        let _ = probe_hold.changed.wait_timeout_while(
                            release,
                            Duration::from_secs(5),
                            |value| !*value,
                        );
                    }
                }
                gst::PadProbeReturn::Ok
            });
        }
        None
    });
}

fn observe_bus(
    pipeline: &gst::Pipeline,
    closed: &Arc<AtomicUsize>,
    eos: &Arc<AtomicBool>,
) -> anyhow::Result<()> {
    let closed = Arc::clone(closed);
    let eos = Arc::clone(eos);
    pipeline
        .bus()
        .ok_or_else(|| anyhow::anyhow!("pipeline bus"))?
        .set_sync_handler(move |_, message| {
            match message.view() {
                gst::MessageView::Eos(_) => eos.store(true, Ordering::Release),
                gst::MessageView::Element(element)
                    if element
                        .structure()
                        .is_some_and(|value| value.name() == "splitmuxsink-fragment-closed") =>
                {
                    closed.fetch_add(1, Ordering::AcqRel);
                }
                _ => {}
            }
            gst::BusSyncReply::Pass
        });
    Ok(())
}

fn release_hold(hold: &Hold) {
    if let Ok(mut release) = hold.release.lock() {
        *release = true;
        hold.changed.notify_all();
    }
}

fn wait_until(timeout: Duration, condition: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}
