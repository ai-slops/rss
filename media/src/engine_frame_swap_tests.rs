use super::*;
use crate::engine_frame_fixture::Fixture;
use crate::protocol::{FitMode, NormalizedCrop, PlacementContent, PlacementId};

#[test]
fn applied_timestamp_is_the_first_new_pixel_frame_not_the_previous_frame() -> anyhow::Result<()> {
    let fixture = Fixture::new(10)?;
    for _ in 0..20 {
        if fixture.frame()?.1 == [255, 0, 0] {
            break;
        }
    }
    let placement = CompositionPlacement {
        id: PlacementId::new("green")?,
        content: PlacementContent::Solid { argb: 0xff00_ff00 },
        x: 0,
        y: 0,
        width: 16,
        height: 16,
        crop: NormalizedCrop {
            left_milli: 0,
            top_milli: 0,
            right_milli: 0,
            bottom_milli: 0,
        },
        fit: FitMode::Stretch,
        opacity_milli: 1000,
        z_order: 1,
    };
    let mut swap = FrameSwap::arm(
        &fixture.compositor,
        vec![
            PadChange::hidden(fixture.pad("sink_0")?),
            PadChange::placement(fixture.pad("sink_1")?, &placement),
        ],
    )
    .map_err(anyhow::Error::msg)?;
    let SwapResult::Confirmed(applied) = swap
        .wait(Duration::from_secs(2))
        .map_err(anyhow::Error::msg)?
    else {
        anyhow::bail!("swap cancelled");
    };
    swap.quiesce().map_err(anyhow::Error::msg)?;
    let mut first_green = None;
    for _ in 0..20 {
        let (pts, pixel) = fixture.frame()?;
        if pixel == [0, 255, 0] {
            first_green = Some(pts);
            break;
        }
    }
    assert_eq!(
        Some(applied.value()),
        first_green,
        "application time must match the first actually rendered green frame"
    );
    Ok(())
}

#[test]
fn timeout_before_any_frame_cannot_apply_a_later_hidden_take() -> anyhow::Result<()> {
    let fixture = Fixture::new(10)?;
    let _ = fixture.frame()?;
    fixture.pipeline.set_state(gst::State::Paused)?;
    let (_, state, _) = fixture.pipeline.state(gst::ClockTime::from_seconds(2));
    assert_eq!(state, gst::State::Paused);
    let mut swap = FrameSwap::arm(
        &fixture.compositor,
        vec![PadChange::hidden(fixture.pad("sink_0")?)],
    )
    .map_err(anyhow::Error::msg)?;
    assert!(matches!(
        swap.wait(Duration::from_millis(20))
            .map_err(anyhow::Error::msg)?,
        SwapResult::Cancelled
    ));
    swap.quiesce().map_err(anyhow::Error::msg)?;
    fixture.pipeline.set_state(gst::State::Playing)?;
    for _ in 0..4 {
        assert_ne!(fixture.frame()?.1, [0, 255, 0]);
    }
    assert_eq!(
        fixture.pad("sink_0")?.property::<f64>("alpha").to_bits(),
        1.0_f64.to_bits()
    );
    Ok(())
}

#[test]
fn timeout_after_property_staging_drops_new_pixels_and_restores_exact_old_alpha()
-> anyhow::Result<()> {
    let fixture = Fixture::new(2)?;
    fixture.pad("sink_0")?.set_property("alpha", 0.5_f64);
    let _ = fixture.frame()?;
    let green = CompositionPlacement {
        id: PlacementId::new("green")?,
        content: PlacementContent::Solid { argb: 0xff00_ff00 },
        x: 0,
        y: 0,
        width: 16,
        height: 16,
        crop: NormalizedCrop {
            left_milli: 0,
            top_milli: 0,
            right_milli: 0,
            bottom_milli: 0,
        },
        fit: FitMode::Stretch,
        opacity_milli: 1000,
        z_order: 1,
    };
    let mut swap = FrameSwap::arm(
        &fixture.compositor,
        vec![
            PadChange::hidden(fixture.pad("sink_0")?),
            PadChange::placement(fixture.pad("sink_1")?, &green),
        ],
    )
    .map_err(anyhow::Error::msg)?;
    let deadline = Instant::now() + Duration::from_secs(2);
    while !swap.staged() {
        anyhow::ensure!(Instant::now() < deadline, "no staging boundary");
        std::thread::sleep(Duration::from_millis(1));
    }
    fixture.pipeline.set_state(gst::State::Paused)?;
    let (_, state, _) = fixture.pipeline.state(gst::ClockTime::from_seconds(2));
    assert_eq!(state, gst::State::Paused);
    assert!(matches!(
        swap.wait(Duration::from_millis(20))
            .map_err(anyhow::Error::msg)?,
        SwapResult::Cancelled
    ));
    assert!(
        swap.quiesce().is_err(),
        "a cancelled staged swap must remain fenced until rollback"
    );
    fixture.pipeline.set_state(gst::State::Playing)?;
    let deadline = Instant::now() + Duration::from_secs(3);
    while !swap.settled() {
        anyhow::ensure!(Instant::now() < deadline, "rollback stalled");
        std::thread::sleep(Duration::from_millis(1));
    }
    swap.quiesce().map_err(anyhow::Error::msg)?;
    assert_eq!(
        fixture.pad("sink_0")?.property::<f64>("alpha").to_bits(),
        0.5_f64.to_bits()
    );
    assert_eq!(
        fixture.pad("sink_1")?.property::<f64>("alpha").to_bits(),
        0.0_f64.to_bits()
    );
    for _ in 0..4 {
        assert_ne!(
            fixture.frame()?.1,
            [0, 255, 0],
            "cancelled green frame escaped"
        );
    }
    Ok(())
}

#[test]
fn failed_layer_preparation_releases_bins_and_request_pads_without_changing_pixels()
-> anyhow::Result<()> {
    let fixture = Fixture::new(10)?;
    let bins = fixture.pipeline.children().len();
    let pads = fixture.compositor.sink_pads().len();
    let mut renderer = CompositionState::new("mix");
    let placements = [
        CompositionPlacement {
            id: PlacementId::new("staged")?,
            content: PlacementContent::Solid { argb: 0xff00_ff00 },
            x: 0,
            y: 0,
            width: 16,
            height: 16,
            crop: NormalizedCrop {
                left_milli: 0,
                top_milli: 0,
                right_milli: 0,
                bottom_milli: 0,
            },
            fit: FitMode::Stretch,
            opacity_milli: 1000,
            z_order: 1,
        },
        CompositionPlacement {
            id: PlacementId::new("unavailable")?,
            content: PlacementContent::Track {
                track_id: crate::protocol::TrackId::new("missing")?,
            },
            x: 0,
            y: 0,
            width: 16,
            height: 16,
            crop: NormalizedCrop {
                left_milli: 0,
                top_milli: 0,
                right_milli: 0,
                bottom_milli: 0,
            },
            fit: FitMode::Stretch,
            opacity_milli: 1000,
            z_order: 2,
        },
    ];
    for _ in 0..3 {
        assert!(
            renderer
                .replace(
                    &fixture.pipeline,
                    Layout {
                        placements: &placements,
                        tracks: &std::collections::HashMap::new()
                    }
                )
                .is_err()
        );
        assert_eq!(fixture.pipeline.children().len(), bins);
        assert_eq!(fixture.compositor.sink_pads().len(), pads);
    }
    for _ in 0..4 {
        assert_ne!(fixture.frame()?.1, [0, 255, 0]);
    }
    Ok(())
}
