use live_production_media::protocol::{
    CompositionPlacement, FitMode, NormalizedCrop, PlacementContent, PlacementId, TrackId,
};
use num_traits::ToPrimitive;

use crate::{
    error::{AppError, Result},
    model::{Asset, Content, Fit, Scene},
};

pub(crate) fn composition_placements(
    scene: &Scene,
    assets: &[Asset],
) -> Result<Vec<CompositionPlacement>> {
    scene
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let opacity_milli = if item.visible {
                milli(item.opacity)?
            } else {
                0
            };
            let z_order = u32::try_from(scene.items.len().saturating_sub(index))
                .map_err(|error| AppError::Internal(error.into()))?;
            Ok(CompositionPlacement {
                id: PlacementId::new(&item.id.0)
                    .map_err(|error| AppError::Invalid(error.to_string()))?,
                content: content(&item.content, assets)?,
                x: coordinate(item.rect.x)?,
                y: coordinate(item.rect.y)?,
                width: coordinate(item.rect.width)?,
                height: coordinate(item.rect.height)?,
                crop: NormalizedCrop {
                    left_milli: milli(item.crop.left)?,
                    top_milli: milli(item.crop.top)?,
                    right_milli: milli(item.crop.right)?,
                    bottom_milli: milli(item.crop.bottom)?,
                },
                fit: match item.fit {
                    Fit::Contain => FitMode::Contain,
                    Fit::Cover => FitMode::Cover,
                    Fit::Stretch => FitMode::Stretch,
                },
                opacity_milli,
                z_order,
            })
        })
        .collect()
}

fn content(value: &Content, assets: &[Asset]) -> Result<PlacementContent> {
    match value {
        Content::Track { track_id } => Ok(PlacementContent::Track {
            track_id: TrackId::new(&track_id.0)
                .map_err(|error| AppError::Invalid(error.to_string()))?,
        }),
        Content::Solid { color } => Ok(PlacementContent::Solid { argb: argb(color)? }),
        Content::Text {
            text,
            color,
            font_size,
        } => Ok(PlacementContent::Text {
            text: text.clone(),
            argb: argb(color)?,
            font_size: *font_size,
        }),
        Content::Image { asset_id, version } => {
            let asset = assets
                .iter()
                .find(|asset| &asset.id == asset_id && &asset.version == version)
                .ok_or_else(|| AppError::Invalid("Image asset version no longer exists.".into()))?;
            Ok(PlacementContent::Image {
                path: asset.path.clone(),
            })
        }
    }
}

fn argb(value: &str) -> Result<u32> {
    let rgb = value
        .strip_prefix('#')
        .ok_or_else(|| AppError::Invalid("Invalid scene color.".into()))?;
    u32::from_str_radix(rgb, 16)
        .map(|color| 0xff00_0000 | color)
        .map_err(|_| AppError::Invalid("Invalid scene color.".into()))
}

fn milli(value: f64) -> Result<u16> {
    (value * 1_000.0)
        .round()
        .to_u16()
        .ok_or_else(|| AppError::Invalid("Scene crop or opacity is out of range.".into()))
}

fn coordinate(value: f64) -> Result<i32> {
    value
        .round()
        .to_i32()
        .ok_or_else(|| AppError::Invalid("Scene geometry is out of range.".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Crop, Id, Rect, SceneItem};

    #[test]
    fn same_track_can_be_placed_twice_with_distinct_ids_and_transforms() -> Result<()> {
        // Given
        let track_id = Id::new();
        let item = |name: &str, x: f64| SceneItem {
            id: Id(name.to_owned()),
            name: name.to_owned(),
            visible: true,
            locked: false,
            rect: Rect {
                x,
                y: 0.0,
                width: 320.0,
                height: 180.0,
            },
            crop: Crop {
                left: 0.1,
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
            },
            fit: Fit::Cover,
            opacity: 0.75,
            content: Content::Track {
                track_id: track_id.clone(),
            },
        };
        let scene = Scene {
            id: Id::new(),
            name: "Double camera".into(),
            revision: "1".into(),
            items: vec![item("left", 0.0), item("right", 320.0)],
        };

        // When
        let placements = composition_placements(&scene, &[])?;

        // Then
        assert_eq!(placements.len(), 2);
        assert_ne!(placements[0].id, placements[1].id);
        assert_eq!(placements[0].x, 0);
        assert_eq!(placements[1].x, 320);
        assert_eq!(placements[0].opacity_milli, 750);
        Ok(())
    }
}
