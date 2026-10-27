use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

const RUNNING: u8 = 0;
const FLUSHING: u8 = 1;

pub struct EpochControl {
    authorized_generation: AtomicU64,
    phase: AtomicU8,
}

impl EpochControl {
    pub const fn new(initial_generation: u64) -> Self {
        Self {
            authorized_generation: AtomicU64::new(initial_generation),
            phase: AtomicU8::new(RUNNING),
        }
    }

    pub fn generation_for_buffer(&self) -> Option<u64> {
        (self.phase.load(Ordering::Acquire) == RUNNING)
            .then(|| self.authorized_generation.load(Ordering::Acquire))
    }

    pub fn authorized_generation(&self) -> u64 {
        self.authorized_generation.load(Ordering::Acquire)
    }

    pub fn begin_flush(&self) {
        self.phase.store(FLUSHING, Ordering::Release);
    }

    pub fn authorize(&self, generation: u64) {
        self.authorized_generation
            .store(generation, Ordering::Release);
    }

    pub fn resume(&self) {
        self.phase.store(RUNNING, Ordering::Release);
    }
}

pub struct EpochState {
    offset: Option<i128>,
    latched_generation: u64,
    last_raw: Option<u64>,
    last_normalized_end: u64,
    pub segment_pending: bool,
}

impl EpochState {
    pub const fn new(initial_generation: u64) -> Self {
        Self {
            offset: None,
            latched_generation: initial_generation,
            last_raw: None,
            last_normalized_end: 0,
            segment_pending: true,
        }
    }

    pub fn normalize(
        &mut self,
        raw: u64,
        duration: u64,
        running: u64,
        discontinuity: bool,
        authorized_generation: u64,
    ) -> (u64, i128, u64) {
        let reset = self.last_raw.is_some_and(|previous| raw < previous);
        let boundary = self.segment_pending || discontinuity || reset;
        if boundary {
            self.latched_generation = authorized_generation;
            let anchor = running.max(self.last_normalized_end);
            self.offset = Some(i128::from(anchor) - i128::from(raw));
            self.segment_pending = false;
        }
        let offset = self.offset.unwrap_or_default();
        let normalized = apply_offset(raw, offset).unwrap_or(self.last_normalized_end);
        self.last_raw = Some(raw);
        self.last_normalized_end = normalized.saturating_add(duration);
        (normalized, offset, self.latched_generation)
    }
}

fn apply_offset(value: u64, offset: i128) -> Option<u64> {
    u64::try_from(i128::from(value) + offset).ok()
}

#[cfg(test)]
mod tests {
    use super::{EpochControl, EpochState};

    #[test]
    fn admission_highwater_cannot_authorize_before_flush_completion() {
        let control = EpochControl::new(1);
        let mut state = EpochState::new(1);
        let first = state.normalize(
            100,
            10,
            1_000,
            false,
            control.generation_for_buffer().unwrap(),
        );
        let preflush_discont = state.normalize(
            110,
            10,
            1_010,
            true,
            control.generation_for_buffer().unwrap(),
        );
        control.begin_flush();
        assert!(control.generation_for_buffer().is_none());
        control.authorize(2);
        control.resume();
        let postflush =
            state.normalize(5, 10, 1_020, true, control.generation_for_buffer().unwrap());
        assert_eq!((first.0, first.2), (1_000, 1));
        assert_eq!((preflush_discont.0, preflush_discont.2), (1_010, 1));
        assert_eq!((postflush.0, postflush.2), (1_020, 2));
    }
}
