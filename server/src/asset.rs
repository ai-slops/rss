use crate::error::{AppError, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::io::Cursor;

pub struct ParsedImage {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
    pub extension: &'static str,
}

pub fn parse_image(encoded: &str, declared_mime: &str) -> Result<ParsedImage> {
    if encoded.len() > 5_600_000 {
        return Err(AppError::Invalid(
            "Upload a PNG or JPEG image smaller than 4 MiB.".into(),
        ));
    }
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| AppError::Invalid("The image upload is not valid base64.".into()))?;
    if bytes.len() > 4_194_304 {
        return Err(AppError::Invalid(
            "Upload a PNG or JPEG image smaller than 4 MiB.".into(),
        ));
    }
    let mut reader = image::ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|_| AppError::Invalid("Unable to inspect the image.".into()))?;
    let (mime, extension) = match reader.format() {
        Some(image::ImageFormat::Png) => ("image/png", "png"),
        Some(image::ImageFormat::Jpeg) => ("image/jpeg", "jpg"),
        _ => {
            return Err(AppError::Invalid(
                "Only static PNG and JPEG images are supported.".into(),
            ));
        }
    };
    if declared_mime != mime {
        return Err(AppError::Invalid(
            "The image bytes do not match the declared media type.".into(),
        ));
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(67_108_864);
    reader.limits(limits);
    reader.decode().map_err(|_| {
        AppError::Invalid(
            "The image is corrupt or exceeds the 4096-pixel/64-MiB decode budget.".into(),
        )
    })?;
    Ok(ParsedImage {
        bytes,
        mime,
        extension,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_png() -> Vec<u8> {
        let image = image::DynamicImage::new_rgba8(2, 2);
        let mut output = Cursor::new(Vec::new());
        image
            .write_to(&mut output, image::ImageFormat::Png)
            .expect("encode fixture");
        output.into_inner()
    }

    #[test]
    fn decoded_png_is_accepted_with_exact_media_type() {
        let bytes = valid_png();
        let parsed = parse_image(&STANDARD.encode(&bytes), "image/png").expect("valid image");
        assert_eq!(parsed.bytes, bytes);
        assert_eq!(parsed.extension, "png");
    }

    #[test]
    fn corrupt_pixels_are_rejected_not_only_header_checked() {
        let mut bytes = valid_png();
        bytes.truncate(40);
        assert!(parse_image(&STANDARD.encode(bytes), "image/png").is_err());
    }

    #[test]
    fn conflicting_declared_type_is_rejected() {
        assert!(parse_image(&STANDARD.encode(valid_png()), "image/jpeg").is_err());
    }

    #[test]
    fn excessive_dimensions_are_rejected_before_large_pixel_allocation() {
        let image = image::DynamicImage::new_rgba8(8192, 1);
        let mut output = Cursor::new(Vec::new());
        image
            .write_to(&mut output, image::ImageFormat::Png)
            .expect("wide fixture");
        assert!(parse_image(&STANDARD.encode(output.into_inner()), "image/png").is_err());
    }
}
