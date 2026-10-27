// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Reading and writing image files.

use std::io::Cursor;
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageFormat, RgbaImage};
use tiny_skia::Pixmap;

use super::color::Rgba;
use super::{raster, transform};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum FileFormat {
    Png,
    Jpeg,
    Bmp,
    Gif,
    Tiff,
    Webp,
    Ico,
}

impl FileFormat {
    pub const ALL: [FileFormat; 7] = [
        FileFormat::Png,
        FileFormat::Jpeg,
        FileFormat::Bmp,
        FileFormat::Gif,
        FileFormat::Tiff,
        FileFormat::Webp,
        FileFormat::Ico,
    ];

    /// Name shown in "Save as" menus and file dialogs.
    pub fn label(self) -> &'static str {
        match self {
            FileFormat::Png => "PNG picture",
            FileFormat::Jpeg => "JPEG picture",
            FileFormat::Bmp => "BMP picture",
            FileFormat::Gif => "GIF picture",
            FileFormat::Tiff => "TIFF picture",
            FileFormat::Webp => "WEBP picture",
            FileFormat::Ico => "ICO icon",
        }
    }

    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            FileFormat::Png => &["png"],
            FileFormat::Jpeg => &["jpg", "jpeg", "jpe", "jfif"],
            FileFormat::Bmp => &["bmp", "dib"],
            FileFormat::Gif => &["gif"],
            FileFormat::Tiff => &["tif", "tiff"],
            FileFormat::Webp => &["webp"],
            FileFormat::Ico => &["ico"],
        }
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        let ext = ext.to_ascii_lowercase();
        Self::ALL.into_iter().find(|f| f.extensions().contains(&ext.as_str()))
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension().and_then(|e| e.to_str()).and_then(Self::from_extension)
    }

    /// Whether transparency survives saving in this format.
    pub fn keeps_transparency(self) -> bool {
        !matches!(self, FileFormat::Jpeg | FileFormat::Bmp)
    }

    fn image_format(self) -> ImageFormat {
        match self {
            FileFormat::Png => ImageFormat::Png,
            FileFormat::Jpeg => ImageFormat::Jpeg,
            FileFormat::Bmp => ImageFormat::Bmp,
            FileFormat::Gif => ImageFormat::Gif,
            FileFormat::Tiff => ImageFormat::Tiff,
            FileFormat::Webp => ImageFormat::WebP,
            FileFormat::Ico => ImageFormat::Ico,
        }
    }
}

/// Every extension OpenBrush can open.
pub fn openable_extensions() -> Vec<&'static str> {
    FileFormat::ALL.iter().flat_map(|f| f.extensions().iter().copied()).collect()
}

fn to_pixmap(img: DynamicImage) -> Result<Pixmap, String> {
    let rgba = img.into_rgba8();
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return Err("The image is empty.".into());
    }
    if w > super::document::MAX_DIMENSION || h > super::document::MAX_DIMENSION {
        return Err(format!(
            "The image is too large ({w} × {h}). The maximum is {0} × {0} pixels.",
            super::document::MAX_DIMENSION
        ));
    }
    raster::from_rgba8(w, h, rgba.as_raw()).ok_or_else(|| "Could not read the image data.".into())
}

pub fn decode(bytes: &[u8]) -> Result<Pixmap, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("This file isn't a supported picture ({e})."))?;
    to_pixmap(img)
}

pub fn load(path: &Path) -> Result<Pixmap, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Couldn't read {}: {e}", path.display()))?;
    decode(&bytes)
}

/// Encodes an image. Formats without transparency are flattened onto white.
pub fn encode(p: &Pixmap, format: FileFormat) -> Result<Vec<u8>, String> {
    let source = if format.keeps_transparency() {
        p.clone()
    } else {
        raster::flatten_onto(p, Rgba::WHITE)
    };
    let source = if format == FileFormat::Ico && (source.width() > 256 || source.height() > 256) {
        let scale = 256.0 / source.width().max(source.height()) as f32;
        let w = ((source.width() as f32 * scale).round() as u32).max(1);
        let h = ((source.height() as f32 * scale).round() as u32).max(1);
        transform::scale(&source, w, h, true)
    } else {
        source
    };
    let rgba = RgbaImage::from_raw(source.width(), source.height(), raster::to_rgba8(&source))
        .ok_or("Could not prepare the image data.")?;
    let img = DynamicImage::ImageRgba8(rgba);
    let mut out = Cursor::new(Vec::new());
    let result = match format {
        FileFormat::Jpeg => img.to_rgb8().write_with_encoder(JpegEncoder::new_with_quality(&mut out, 92)),
        FileFormat::Bmp => DynamicImage::ImageRgb8(img.to_rgb8()).write_to(&mut out, ImageFormat::Bmp),
        other => img.write_to(&mut out, other.image_format()),
    };
    result.map_err(|e| format!("Couldn't save as {}: {e}", format.label()))?;
    Ok(out.into_inner())
}

/// Writes the image to disk and returns the file size in bytes.
pub fn save(path: &Path, p: &Pixmap, format: FileFormat) -> Result<u64, String> {
    let bytes = encode(p, format)?;
    std::fs::write(path, &bytes).map_err(|e| format!("Couldn't save {}: {e}", path.display()))?;
    Ok(bytes.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::raster::{filled, get_pixel, set_pixel};

    fn sample() -> Pixmap {
        let mut p = filled(8, 6, Rgba::rgb(255, 0, 0));
        set_pixel(&mut p, 0, 0, Rgba::rgb(0, 0, 255));
        set_pixel(&mut p, 7, 5, Rgba::TRANSPARENT);
        p
    }

    #[test]
    fn every_format_round_trips() {
        for format in FileFormat::ALL {
            let bytes = encode(&sample(), format).unwrap_or_else(|e| panic!("{format:?}: {e}"));
            let back = decode(&bytes).unwrap_or_else(|e| panic!("{format:?}: {e}"));
            assert_eq!((back.width(), back.height()), (8, 6), "{format:?}");
            let c = get_pixel(&back, 4, 3).unwrap();
            assert!(c.r > 200 && c.g < 60 && c.b < 60, "{format:?} centre pixel {c:?}");
            let corner = get_pixel(&back, 7, 5).unwrap();
            if format.keeps_transparency() {
                assert_eq!(corner.a, 0, "{format:?} lost transparency");
            } else {
                assert!(corner.a == 255 && corner.r > 240 && corner.g > 240, "{format:?} {corner:?}");
            }
        }
    }

    #[test]
    fn lossless_formats_are_exact() {
        for format in [FileFormat::Png, FileFormat::Bmp, FileFormat::Tiff, FileFormat::Webp] {
            let back = decode(&encode(&sample(), format).unwrap()).unwrap();
            assert_eq!(get_pixel(&back, 0, 0), Some(Rgba::rgb(0, 0, 255)), "{format:?}");
        }
    }

    #[test]
    fn large_icons_are_scaled_down() {
        let big = filled(512, 300, Rgba::BLACK);
        let back = decode(&encode(&big, FileFormat::Ico).unwrap()).unwrap();
        assert_eq!((back.width(), back.height()), (256, 150));
    }

    #[test]
    fn format_from_path() {
        assert_eq!(FileFormat::from_path(Path::new("a/b.JPG")), Some(FileFormat::Jpeg));
        assert_eq!(FileFormat::from_path(Path::new("x.tiff")), Some(FileFormat::Tiff));
        assert_eq!(FileFormat::from_path(Path::new("x.txt")), None);
        assert_eq!(FileFormat::from_path(Path::new("noext")), None);
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(decode(b"definitely not an image").is_err());
    }

    #[test]
    fn save_and_load_from_disk() {
        let dir = std::env::temp_dir().join(format!("openbrush-io-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.png");
        let size = save(&path, &sample(), FileFormat::Png).unwrap();
        assert_eq!(size, std::fs::metadata(&path).unwrap().len());
        let back = load(&path).unwrap();
        assert_eq!(get_pixel(&back, 0, 0), Some(Rgba::rgb(0, 0, 255)));
        std::fs::remove_dir_all(&dir).ok();
    }
}
