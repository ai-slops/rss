use std::collections::HashSet;

use crate::protocol::{CompositionPlacement, FitMode, NormalizedCrop};

const SOURCE_WIDTH: i32 = 1280;
const SOURCE_HEIGHT: i32 = 720;

pub fn transform_chain(placement: &CompositionPlacement) -> String {
    let crop = crop_pixels(placement.crop);
    let crop = format!(
        "videocrop left={} top={} right={} bottom={} ! ",
        crop.0, crop.1, crop.2, crop.3
    );
    match placement.fit {
        FitMode::Contain => format!(
            "{crop}videoscale add-borders=true ! video/x-raw,width={},height={},pixel-aspect-ratio=1/1 ! videoconvert",
            placement.width, placement.height
        ),
        FitMode::Cover => format!(
            "{crop}aspectratiocrop aspect-ratio={}/{} ! videoscale ! video/x-raw,width={},height={},pixel-aspect-ratio=1/1 ! videoconvert",
            placement.width, placement.height, placement.width, placement.height
        ),
        FitMode::Stretch => format!(
            "{crop}videoscale ! video/x-raw,width={},height={},pixel-aspect-ratio=1/1 ! videoconvert",
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

fn crop_pixels(crop: NormalizedCrop) -> (i32, i32, i32, i32) {
    (
        SOURCE_WIDTH * i32::from(crop.left_milli) / 1_000,
        SOURCE_HEIGHT * i32::from(crop.top_milli) / 1_000,
        SOURCE_WIDTH * i32::from(crop.right_milli) / 1_000,
        SOURCE_HEIGHT * i32::from(crop.bottom_milli) / 1_000,
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
