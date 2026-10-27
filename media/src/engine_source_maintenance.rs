use std::time::Instant;

use gst::prelude::*;
use gstreamer as gst;

use crate::engine_source_runtime::SourceRuntime;

impl SourceRuntime {
    pub fn pending_admission_expired(&self, now: Instant) -> bool {
        self.admission
            .as_ref()
            .is_some_and(|admission| admission.pending_expired(now))
    }

    pub fn recovery_generation(&self) -> u64 {
        crate::engine_source_generation::after_pending(
            self.generation,
            self.admission
                .as_ref()
                .map(|admission| admission.generation()),
        )
    }

    pub fn current_generation(&self) -> u64 {
        self.epochs.authorized_generation()
    }

    pub fn maintain_epoch(&self) -> Result<(), String> {
        let Some(admission) = &self.admission else {
            return Ok(());
        };
        if !admission.take_epoch_reset() {
            return Ok(());
        }
        let generation = admission.generation();
        if generation <= self.epochs.authorized_generation() {
            return Ok(());
        }
        let listener = self
            .srt_listener_name
            .as_ref()
            .and_then(|name| self.bin.by_name(name))
            .ok_or_else(|| "source epoch listener is unavailable".to_owned())?;
        self.epochs.begin_flush();
        let started = listener.send_event(gst::event::FlushStart::new());
        let stopped = started && listener.send_event(gst::event::FlushStop::new(false));
        if stopped {
            self.epochs.complete_flush(generation);
            Ok(())
        } else {
            Err("source epoch flush did not complete".to_owned())
        }
    }
}
