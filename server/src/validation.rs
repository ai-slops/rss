use crate::{
    error::{AppError, Result},
    model::*,
};
use std::collections::HashSet;

pub fn name(value: &str) -> Result<()> {
    if value.trim().is_empty() || value.chars().count() > 100 || value.chars().any(char::is_control)
    {
        return Err(AppError::Invalid(
            "Use a name containing 1–100 printable characters.".into(),
        ));
    }
    Ok(())
}

pub fn revision(expected: Option<&str>, actual: &str) -> Result<()> {
    if expected != Some(actual) {
        return Err(AppError::Conflict(format!(
            "This object changed (current revision {actual}). Refresh and recover your local draft before applying again."
        )));
    }
    Ok(())
}

pub fn next_revision(current: &str) -> Result<String> {
    let value = current
        .parse::<u64>()
        .map_err(|_| AppError::Invalid("Invalid revision".into()))?;
    value
        .checked_add(1)
        .map(|next| next.to_string())
        .ok_or_else(|| AppError::Invalid("Revision exhausted".into()))
}

pub fn scene(scene: &Scene, project: &Project) -> Result<()> {
    name(&scene.name)?;
    if scene.items.len() > 32 {
        return Err(AppError::Invalid(
            "The software profile supports at most 32 scene placements.".into(),
        ));
    }
    let mut ids = HashSet::new();
    for item in &scene.items {
        if !ids.insert(&item.id) {
            return Err(AppError::Invalid(
                "Every placement needs a distinct ID.".into(),
            ));
        }
        name(&item.name)?;
        let rect = &item.rect;
        let coordinates = [rect.x, rect.y, rect.width, rect.height];
        if coordinates
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 16384.0)
            || rect.width < 1.0
            || rect.height < 1.0
        {
            return Err(AppError::Invalid(
                "Placement geometry must be finite and between 1 and 16384 pixels in size.".into(),
            ));
        }
        let crop = &item.crop;
        if [crop.left, crop.right, crop.top, crop.bottom, item.opacity]
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            || crop.left + crop.right >= 1.0
            || crop.top + crop.bottom >= 1.0
        {
            return Err(AppError::Invalid(
                "Crop must retain visible source pixels; opacity must be between 0 and 1.".into(),
            ));
        }
        match &item.content {
            Content::Track { track_id } => {
                if !project
                    .sources
                    .iter()
                    .flat_map(|s| &s.tracks)
                    .any(|t| &t.id == track_id && t.kind == TrackKind::Video)
                {
                    return Err(AppError::Invalid(
                        "The placement must reference an existing video track.".into(),
                    ));
                }
            }
            Content::Solid { color } => color_value(color)?,
            Content::Text {
                text,
                color,
                font_size,
            } => {
                color_value(color)?;
                if text.len() > 4096 || !(8..=256).contains(font_size) {
                    return Err(AppError::Invalid(
                        "Text is limited to 4096 bytes and font sizes 8–256.".into(),
                    ));
                }
            }
            Content::Image { asset_id, version } => {
                if !project
                    .assets
                    .iter()
                    .any(|a| &a.id == asset_id && &a.version == version)
                {
                    return Err(AppError::Invalid(
                        "Select a saved image asset version.".into(),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn color_value(value: &str) -> Result<()> {
    if value.len() != 7
        || !value.starts_with('#')
        || !value.bytes().skip(1).all(|b| b.is_ascii_hexdigit())
    {
        return Err(AppError::Invalid(
            "Use a six-digit hexadecimal color.".into(),
        ));
    }
    Ok(())
}

pub fn routes(routes: &[AudioRoute], project: &Project) -> Result<()> {
    let mut ids = HashSet::new();
    for route in routes {
        if !ids.insert(&route.track_id)
            || !project
                .sources
                .iter()
                .flat_map(|s| &s.tracks)
                .any(|t| t.id == route.track_id && t.kind == TrackKind::Audio)
        {
            return Err(AppError::Invalid(
                "Route each existing audio track at most once.".into(),
            ));
        }
        if !route.gain_db.is_finite() || !(-60.0..=12.0).contains(&route.gain_db) {
            return Err(AppError::Invalid(
                "Gain must be between −60 and +12 dB.".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_revision_is_rejected_instead_of_overwriting() {
        let result = revision(Some("3"), "4");
        assert!(matches!(result, Err(AppError::Conflict(_))));
    }

    #[test]
    fn duplicate_audio_route_is_rejected() {
        let track_id = Id::new();
        let mut project = Project::empty();
        project.sources.push(Source {
            id: Id::new(),
            name: "Mic".into(),
            enabled: true,
            adapter: SourceAdapter::Synthetic {
                pattern: "smpte".into(),
                frequency_hz: 440,
            },
            tracks: vec![Track {
                id: track_id.clone(),
                kind: TrackKind::Audio,
                correction_ms: 0,
            }],
            record_on_connect: false,
        });
        let route = AudioRoute {
            track_id,
            gain_db: 0.0,
            muted: false,
            routed: true,
        };
        let result = routes(&[route.clone(), route], &project);
        assert!(matches!(result, Err(AppError::Invalid(_))));
    }
}
