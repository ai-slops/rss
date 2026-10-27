use crate::{engine_pad_values::PadChange, protocol::Nanoseconds};
use gstreamer::{self as gst, prelude::*};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
enum Phase {
    Armed,
    Staged,
    Confirmed(Nanoseconds),
    CancelledBefore,
    CancelledStaged,
    Restoring,
    RolledBack,
}

#[derive(Debug)]
pub enum SwapResult {
    Confirmed(Nanoseconds),
    Cancelled,
}

pub struct FrameSwap {
    pad: gst::Pad,
    probe: Option<gst::PadProbeId>,
    phase: Arc<Mutex<Phase>>,
    active: Arc<AtomicUsize>,
    notifications: mpsc::Receiver<()>,
}

struct InProbe(Arc<AtomicUsize>);
impl Drop for InProbe {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl FrameSwap {
    pub fn arm(compositor: &gst::Element, changes: Vec<PadChange>) -> Result<Self, String> {
        let pad = compositor
            .static_pad("src")
            .ok_or_else(|| "compositor has no output pad".to_owned())?;
        let phase = Arc::new(Mutex::new(Phase::Armed));
        let active = Arc::new(AtomicUsize::new(0));
        let (notify, notifications) = mpsc::sync_channel(1);
        let callback_phase = Arc::clone(&phase);
        let callback_active = Arc::clone(&active);
        let probe = pad
            .add_probe(
                gst::PadProbeType::BLOCK | gst::PadProbeType::BUFFER,
                move |_, info| {
                    callback_active.fetch_add(1, Ordering::SeqCst);
                    let _active_guard = InProbe(Arc::clone(&callback_active));
                    let Ok(mut phase) = callback_phase.lock() else {
                        return gst::PadProbeReturn::Drop;
                    };
                    let result = match *phase {
                        Phase::Armed => {
                            for change in &changes {
                                change.after.write(&change.pad);
                            }
                            *phase = Phase::Staged;
                            gst::PadProbeReturn::Pass
                        }
                        Phase::Staged => {
                            if let Some(pts) = info.buffer().and_then(|buffer| buffer.pts()) {
                                *phase = Phase::Confirmed(Nanoseconds::new(pts.nseconds()));
                                gst::PadProbeReturn::Pass
                            } else {
                                for change in &changes {
                                    change.before.write(&change.pad);
                                }
                                *phase = Phase::Restoring;
                                gst::PadProbeReturn::Drop
                            }
                        }
                        Phase::CancelledStaged => {
                            for change in &changes {
                                change.before.write(&change.pad);
                            }
                            *phase = Phase::Restoring;
                            gst::PadProbeReturn::Drop
                        }
                        Phase::Restoring => {
                            *phase = Phase::RolledBack;
                            gst::PadProbeReturn::Pass
                        }
                        Phase::Confirmed(_) | Phase::CancelledBefore | Phase::RolledBack => {
                            gst::PadProbeReturn::Pass
                        }
                    };
                    drop(phase);
                    let _ = notify.try_send(());
                    result
                },
            )
            .ok_or_else(|| "could not arm compositor frame boundary".to_owned())?;
        Ok(Self {
            pad,
            probe: Some(probe),
            phase,
            active,
            notifications,
        })
    }

    pub fn wait(&self, timeout: Duration) -> Result<SwapResult, String> {
        let started = Instant::now();
        loop {
            let mut phase = self
                .phase
                .lock()
                .map_err(|_| "compositor transaction failed".to_owned())?;
            match *phase {
                Phase::Confirmed(pts) => return Ok(SwapResult::Confirmed(pts)),
                Phase::CancelledBefore
                | Phase::CancelledStaged
                | Phase::Restoring
                | Phase::RolledBack => return Ok(SwapResult::Cancelled),
                Phase::Armed | Phase::Staged => {}
            }
            if started.elapsed() >= timeout {
                return match *phase {
                    Phase::Armed => {
                        *phase = Phase::CancelledBefore;
                        Ok(SwapResult::Cancelled)
                    }
                    Phase::Staged => {
                        *phase = Phase::CancelledStaged;
                        Ok(SwapResult::Cancelled)
                    }
                    Phase::Confirmed(pts) => Ok(SwapResult::Confirmed(pts)),
                    Phase::CancelledBefore
                    | Phase::CancelledStaged
                    | Phase::Restoring
                    | Phase::RolledBack => Ok(SwapResult::Cancelled),
                };
            }
            drop(phase);
            let _ = self
                .notifications
                .recv_timeout(timeout.saturating_sub(started.elapsed()));
        }
    }

    pub fn settled(&self) -> bool {
        self.phase.lock().is_ok_and(|phase| {
            matches!(
                *phase,
                Phase::Confirmed(_) | Phase::CancelledBefore | Phase::RolledBack
            )
        })
    }

    #[cfg(test)]
    pub fn staged(&self) -> bool {
        self.phase
            .lock()
            .is_ok_and(|phase| matches!(*phase, Phase::Staged))
    }

    pub fn quiesce(&mut self) -> Result<(), String> {
        if !self.settled() {
            return Err(
                "compositor rollback is awaiting a frame; canvas remains fenced".to_owned(),
            );
        }
        if let Some(probe) = self.probe.take() {
            self.pad.remove_probe(probe);
        }
        let started = Instant::now();
        while self.active.load(Ordering::SeqCst) != 0 {
            if started.elapsed() >= Duration::from_secs(1) {
                return Err("compositor callback did not quiesce".to_owned());
            }
            thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }
}

impl Drop for FrameSwap {
    fn drop(&mut self) {
        if let Some(probe) = self.probe.take() {
            self.pad.remove_probe(probe);
        }
    }
}
