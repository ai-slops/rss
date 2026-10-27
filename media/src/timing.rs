use crate::protocol::Milliseconds;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimingLimits {
    maximum_buffer: Milliseconds,
}

impl TimingLimits {
    #[must_use]
    pub const fn new(maximum_buffer: Milliseconds) -> Self {
        Self { maximum_buffer }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimingPlan {
    video_delay: Milliseconds,
    audio_delay: Milliseconds,
}

impl TimingPlan {
    pub fn validate(
        limits: &TimingLimits,
        holdback: Milliseconds,
        video_correction: Milliseconds,
        audio_correction: Milliseconds,
    ) -> Result<Self, TimingRejection> {
        if holdback.value() < 0 || limits.maximum_buffer.value() < 0 {
            return Err(TimingRejection::NegativeBound);
        }

        let video_delay = holdback
            .value()
            .checked_add(video_correction.value())
            .ok_or(TimingRejection::ArithmeticOverflow)?;
        let audio_delay = holdback
            .value()
            .checked_add(audio_correction.value())
            .ok_or(TimingRejection::ArithmeticOverflow)?;
        let earliest = video_delay.min(audio_delay);
        if earliest < 0 {
            return Err(TimingRejection::AdditionalHoldbackRequired(
                Milliseconds::new(earliest.saturating_neg()),
            ));
        }

        let latest = video_delay.max(audio_delay);
        if latest > limits.maximum_buffer.value() {
            return Err(TimingRejection::BufferLimitExceeded(Milliseconds::new(
                latest - limits.maximum_buffer.value(),
            )));
        }

        Ok(Self {
            video_delay: Milliseconds::new(video_delay),
            audio_delay: Milliseconds::new(audio_delay),
        })
    }

    #[must_use]
    pub const fn video_delay(self) -> Milliseconds {
        self.video_delay
    }

    #[must_use]
    pub const fn audio_delay(self) -> Milliseconds {
        self.audio_delay
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TimingRejection {
    #[error("additional Program holdback required: {0:?}")]
    AdditionalHoldbackRequired(Milliseconds),
    #[error("scheduled delay exceeds the bounded buffer by {0:?}")]
    BufferLimitExceeded(Milliseconds),
    #[error("timing arithmetic overflowed")]
    ArithmeticOverflow,
    #[error("holdback and buffer bounds must be non-negative")]
    NegativeBound,
}
