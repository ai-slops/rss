use live_production_media::protocol::Milliseconds;
use live_production_media::timing::{TimingLimits, TimingPlan, TimingRejection};

#[test]
fn negative_correction_is_feasible_when_holdback_covers_earlier_playback() {
    // Given
    let limits = TimingLimits::new(Milliseconds::new(2_000));

    // When
    let plan = TimingPlan::validate(
        &limits,
        Milliseconds::new(500),
        Milliseconds::new(-250),
        Milliseconds::new(100),
    );

    // Then
    assert!(plan.is_ok());
}

#[test]
fn negative_correction_reports_required_holdback_when_history_is_insufficient() {
    // Given
    let limits = TimingLimits::new(Milliseconds::new(2_000));

    // When
    let rejection = TimingPlan::validate(
        &limits,
        Milliseconds::new(100),
        Milliseconds::new(-250),
        Milliseconds::new(0),
    );

    // Then
    assert_eq!(
        rejection,
        Err(TimingRejection::AdditionalHoldbackRequired(
            Milliseconds::new(150)
        ))
    );
}

#[test]
fn correction_is_rejected_when_scheduled_delay_exceeds_bounded_buffer() {
    // Given
    let limits = TimingLimits::new(Milliseconds::new(1_000));

    // When
    let rejection = TimingPlan::validate(
        &limits,
        Milliseconds::new(900),
        Milliseconds::new(200),
        Milliseconds::new(0),
    );

    // Then
    assert_eq!(
        rejection,
        Err(TimingRejection::BufferLimitExceeded(Milliseconds::new(100)))
    );
}
