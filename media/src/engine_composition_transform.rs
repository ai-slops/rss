use std::collections::HashSet;

use crate::protocol::{CompositionPlacement, FitMode, NormalizedCrop};

pub fn transform_chain(placement: &CompositionPlacement) -> String {
    let crop = "videoconvert ! video/x-raw,format=BGRA ! videocrop name=scene_crop ! ";
    match placement.fit {
        FitMode::Contain => format!(
            "{crop}videoscale add-borders=true ! video/x-raw,width={},height={},pixel-aspect-ratio=1/1 ! videoconvert name=scene_output",
            placement.width, placement.height
        ),
        FitMode::Cover => format!(
            "{crop}aspectratiocrop aspect-ratio={}/{} ! videoscale ! video/x-raw,width={},height={},pixel-aspect-ratio=1/1 ! videoconvert name=scene_output",
            placement.width, placement.height, placement.width, placement.height
        ),
        FitMode::Stretch => format!(
            "{crop}videoscale ! video/x-raw,width={},height={},pixel-aspect-ratio=1/1 ! videoconvert name=scene_output",
            placement.width, placement.height
        ),
    }
}

pub fn validate_placements(placements: &[CompositionPlacement]) -> Result<(), String> {
    let mut ids = HashSet::new();
    for placement in placements {
        let crop = placement.crop;
        if !ids.insert(&placement.id)
            || placement.width <= 0
            || placement.height <= 0
            || placement.opacity_milli > 1_000
            || u32::from(crop.left_milli) + u32::from(crop.right_milli) >= 1_000
            || u32::from(crop.top_milli) + u32::from(crop.bottom_milli) >= 1_000
        {
            return Err(format!("invalid placement '{}'", placement.id.as_str()));
        }
    }
    Ok(())
}

pub fn crop_pixels(crop: NormalizedCrop, width: i32, height: i32) -> (i32, i32, i32, i32) {
    (
        width * i32::from(crop.left_milli) / 1_000,
        height * i32::from(crop.top_milli) / 1_000,
        width * i32::from(crop.right_milli) / 1_000,
        height * i32::from(crop.bottom_milli) / 1_000,
    )
}

pub fn element_stem(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() {
                char::from(byte)
            } else {
                '_'
            }
        })
        .collect()
}

pub fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use crate::protocol::{
        CompositionPlacement, FitMode, NormalizedCrop, PlacementContent, PlacementId,
    };

    use super::transform_chain;

    #[test]
    fn transform_converts_to_crop_compatible_video() -> Result<(), crate::protocol::IdentifierError>
    {
        // Given
        let placement = CompositionPlacement {
            id: PlacementId::new("camera")?,
            content: PlacementContent::Solid { argb: 0xff00_0000 },
            x: 0,
            y: 0,
            width: 960,
            height: 540,
            crop: NormalizedCrop {
                left_milli: 0,
                top_milli: 0,
                right_milli: 0,
                bottom_milli: 0,
            },
            fit: FitMode::Contain,
            opacity_milli: 1_000,
            z_order: 1,
        };

        // When
        let description = transform_chain(&placement);

        // Then
        assert!(description.starts_with("videoconvert ! video/x-raw,format=BGRA ! videocrop"));
        Ok(())
    }
}
