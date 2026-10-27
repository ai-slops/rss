use live_production_media::{
    engine::{EngineConfig, MediaEngine},
    protocol::{
        Command, CommandId, CommandKind, CompositionPlacement, EventKind, FitMode, NormalizedCrop,
        OutputId, PlacementContent, PlacementId, ProgramRevision, SourceId, SourceKind, SourceSpec,
        SyntheticPattern, TrackId,
    },
};

#[test]
fn dynamic_composition_places_repeated_track_and_static_content() -> anyhow::Result<()> {
    // Given
    let assets = tempfile::tempdir()?;
    let image_path = assets.path().join("pixel.png");
    std::fs::write(
        &image_path,
        b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x04\x00\x00\x00\xb5\x1c\x0c\x02\x00\x00\x00\x0bIDATx\xdacd\xf8\x0f\x00\x01\x05\x01\x01'\x18\xe3f\x00\x00\x00\x00IEND\xaeB`\x82",
    )?;
    let description = "compositor name=preview_compositor background=black force-live=true ignore-inactive-pads=true ! video/x-raw,width=960,height=540,framerate=30/1 ! fakesink sync=false compositor name=program_compositor background=black force-live=true ignore-inactive-pads=true ! video/x-raw,width=960,height=540,framerate=30/1 ! fakesink sync=false";
    let engine = MediaEngine::spawn(EngineConfig::new(
        description,
        std::iter::empty::<(OutputId, &str)>(),
    ))?;
    let video_track_id = TrackId::new("camera-video")?;
    let added = engine.execute(Command {
        id: CommandId::new("dynamic-source")?,
        expected_engine_run_id: None,
        kind: CommandKind::AddSource {
            source: SourceSpec {
                id: SourceId::new("camera")?,
                video_track_id: video_track_id.clone(),
                audio_track_id: TrackId::new("camera-audio")?,
                kind: SourceKind::Synthetic {
                    pattern: SyntheticPattern::Smpte,
                    frequency_hz: 440,
                },
            },
        },
    })?;
    let revision = ProgramRevision::new("dynamic-scene-1")?;
    let placements = placements(&video_track_id, &image_path)?;

    // When
    let prepared = engine.execute(Command {
        id: CommandId::new("dynamic-prepare")?,
        expected_engine_run_id: Some(added.engine_run_id.clone()),
        kind: CommandKind::PrepareComposition {
            revision: revision.clone(),
            placements,
        },
    })?;
    let taken = engine.execute(Command {
        id: CommandId::new("dynamic-take")?,
        expected_engine_run_id: Some(added.engine_run_id),
        kind: CommandKind::TakePrepared {
            revision: revision.clone(),
            expected_program_revision: None,
        },
    })?;

    // Then
    assert!(
        matches!(prepared.kind, EventKind::Applied { .. }),
        "{prepared:?}"
    );
    assert!(matches!(
        taken.kind,
        EventKind::Applied {
            media_at: Some(_),
            program_revision: Some(ref applied),
            ..
        } if applied == &revision
    ));
    Ok(())
}

#[test]
fn revising_the_same_placement_keeps_logical_identity_without_bin_collisions() -> anyhow::Result<()>
{
    let engine = MediaEngine::spawn(EngineConfig::new(
        "compositor name=preview_compositor force-live=true ignore-inactive-pads=true ! video/x-raw,width=64,height=64,framerate=30/1 ! fakesink sync=false compositor name=program_compositor force-live=true ignore-inactive-pads=true ! video/x-raw,width=64,height=64,framerate=30/1 ! fakesink sync=false",
        std::iter::empty::<(OutputId, &str)>(),
    ))?;
    let original = placement((
        "stable-layer",
        PlacementContent::Solid { argb: 0xff20_4060 },
        (0, 0, 64, 64),
        NormalizedCrop {
            left_milli: 0,
            top_milli: 0,
            right_milli: 0,
            bottom_milli: 0,
        },
        FitMode::Stretch,
        1000,
        0,
    ))?;
    let mut program_revision = None;
    for (index, revision) in ["scene-1", "scene-1", "scene-2"].into_iter().enumerate() {
        let revision = ProgramRevision::new(revision)?;
        let mut edited = original.clone();
        edited.x = i32::from(index == 2);
        let prepared = engine.execute(Command {
            id: CommandId::new(&format!("stable-prepare-{index}"))?,
            expected_engine_run_id: None,
            kind: CommandKind::PrepareComposition {
                revision: revision.clone(),
                placements: vec![edited],
            },
        })?;
        assert!(
            matches!(prepared.kind, EventKind::Applied { .. }),
            "{prepared:?}"
        );
        let taken = engine.execute(Command {
            id: CommandId::new(&format!("stable-take-{index}"))?,
            expected_engine_run_id: Some(prepared.engine_run_id),
            kind: CommandKind::TakePrepared {
                revision: revision.clone(),
                expected_program_revision: program_revision,
            },
        })?;
        assert!(matches!(taken.kind, EventKind::Applied { .. }), "{taken:?}");
        program_revision = Some(revision);
    }
    Ok(())
}

fn placements(
    track_id: &TrackId,
    image_path: &std::path::Path,
) -> anyhow::Result<Vec<CompositionPlacement>> {
    let crop = NormalizedCrop {
        left_milli: 0,
        top_milli: 0,
        right_milli: 0,
        bottom_milli: 0,
    };
    Ok(vec![
        placement((
            "camera-left",
            PlacementContent::Track {
                track_id: track_id.clone(),
            },
            (0, 0, 480, 540),
            crop,
            FitMode::Cover,
            1_000,
            0,
        ))?,
        placement((
            "camera-pip",
            PlacementContent::Track {
                track_id: track_id.clone(),
            },
            (640, 360, 320, 180),
            crop,
            FitMode::Contain,
            750,
            2,
        ))?,
        placement((
            "lower-third",
            PlacementContent::Solid { argb: 0xff20_4060 },
            (0, 480, 960, 60),
            crop,
            FitMode::Stretch,
            900,
            1,
        ))?,
        placement((
            "title",
            PlacementContent::Text {
                text: "Live".to_owned(),
                argb: 0xffff_ffff,
                font_size: 48,
            },
            (0, 0, 320, 90),
            crop,
            FitMode::Contain,
            1_000,
            3,
        ))?,
        placement((
            "logo",
            PlacementContent::Image {
                path: image_path.to_string_lossy().into_owned(),
            },
            (860, 0, 100, 100),
            crop,
            FitMode::Contain,
            1_000,
            4,
        ))?,
    ])
}

type PlacementArgs<'a> = (
    &'a str,
    PlacementContent,
    (i32, i32, i32, i32),
    NormalizedCrop,
    FitMode,
    u16,
    u32,
);

fn placement(args: PlacementArgs<'_>) -> anyhow::Result<CompositionPlacement> {
    let (id, content, (x, y, width, height), crop, fit, opacity_milli, z_order) = args;
    Ok(CompositionPlacement {
        id: PlacementId::new(id)?,
        content,
        x,
        y,
        width,
        height,
        crop,
        fit,
        opacity_milli,
        z_order,
    })
}
