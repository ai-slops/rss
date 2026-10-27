use super::*;
use crate::engine_frame_fixture::Fixture;
use std::sync::Arc;

#[test]
fn failed_timing_boundary_cannot_apply_after_program_resumes() -> anyhow::Result<()> {
    // Given a paused Program with an unchanged source timing control.
    let fixture = Fixture::new(10)?;
    let _ = fixture.frame()?;
    fixture.pipeline.set_state(gst::State::Paused)?;
    let (_, state, _) = fixture.pipeline.state(gst::ClockTime::from_seconds(2));
    assert_eq!(state, gst::State::Paused);
    fixture.compositor.set_state(gst::State::Ready)?;
    let (_, state, _) = fixture.compositor.state(gst::ClockTime::from_seconds(2));
    assert_eq!(state, gst::State::Ready);
    let control = Arc::new(TimingControl::default());

    // When the transaction fails before a Program buffer can arrive.
    let result = apply_at_boundary(
        &fixture.pad("src")?,
        vec![TimingChange {
            element: None,
            control: Some(Arc::clone(&control)),
            offset: 500_000_000,
        }],
        Duration::from_millis(20),
    );
    assert!(result.is_err());
    assert_eq!(control.offset(), 0);

    // Then resuming Program cannot execute the failed command later.
    fixture.pipeline.set_state(gst::State::Playing)?;
    for _ in 0..4 {
        let _ = fixture.frame()?;
    }
    assert_eq!(control.offset(), 0);
    Ok(())
}

#[test]
fn committed_timing_wins_over_a_late_notification() -> anyhow::Result<()> {
    let pts = Nanoseconds::new(123_456);
    let mut phase = TimingPhase::Applied(pts);
    assert_eq!(phase.finish().map_err(anyhow::Error::msg)?, pts);
    Ok(())
}

#[test]
fn successful_timing_boundary_reports_real_program_pts() -> anyhow::Result<()> {
    let fixture = Fixture::new(10)?;
    let _ = fixture.frame()?;
    let control = Arc::new(TimingControl::default());
    let applied = apply_at_boundary(
        &fixture.pad("src")?,
        vec![TimingChange {
            element: None,
            control: Some(Arc::clone(&control)),
            offset: 250_000_000,
        }],
        Duration::from_secs(2),
    )
    .map_err(anyhow::Error::msg)?;
    let observed = (0..10)
        .map(|_| fixture.frame().map(|(pts, _)| pts))
        .collect::<anyhow::Result<Vec<_>>>()?;
    assert!(observed.contains(&applied.value()), "{observed:?}");
    assert_eq!(control.offset(), 250_000_000);
    Ok(())
}
