// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Whole-image transforms: rotate, flip, resize, skew and crop.

use tiny_skia::{FilterQuality, IntRect, Pixmap, PixmapPaint, Transform};

use super::raster::{self, new_pixmap};

fn remap(src: &Pixmap, width: u32, height: u32, map: impl Fn(usize, usize) -> (usize, usize)) -> Pixmap {
    let mut dst = new_pixmap(width, height);
    let sw = src.width() as usize;
    let src_px = src.pixels();
    let dw = width as usize;
    let dst_px = dst.pixels_mut();
    for y in 0..src.height() as usize {
        for x in 0..sw {
            let (dx, dy) = map(x, y);
            dst_px[dy * dw + dx] = src_px[y * sw + x];
        }
    }
    dst
}

pub fn rotate_cw(src: &Pixmap) -> Pixmap {
    let h = src.height() as usize;
    remap(src, src.height(), src.width(), |x, y| (h - 1 - y, x))
}

pub fn rotate_ccw(src: &Pixmap) -> Pixmap {
    let w = src.width() as usize;
    remap(src, src.height(), src.width(), |x, y| (y, w - 1 - x))
}

pub fn rotate_180(src: &Pixmap) -> Pixmap {
    let mut dst = src.clone();
    dst.pixels_mut().reverse();
    dst
}

pub fn flip_horizontal(src: &Pixmap) -> Pixmap {
    let mut dst = src.clone();
    let w = dst.width() as usize;
    for row in dst.pixels_mut().chunks_exact_mut(w) {
        row.reverse();
    }
    dst
}

pub fn flip_vertical(src: &Pixmap) -> Pixmap {
    let w = src.width() as usize;
    let mut dst = new_pixmap(src.width(), src.height());
    for (d, s) in dst
        .pixels_mut()
        .chunks_exact_mut(w)
        .zip(src.pixels().chunks_exact(w).rev())
    {
        d.copy_from_slice(s);
    }
    dst
}

fn draw_scaled(src: &Pixmap, width: u32, height: u32, quality: FilterQuality) -> Pixmap {
    let mut dst = new_pixmap(width, height);
    let sx = width as f32 / src.width() as f32;
    let sy = height as f32 / src.height() as f32;
    dst.draw_pixmap(
        0,
        0,
        src.as_ref(),
        &PixmapPaint {
            quality,
            blend_mode: tiny_skia::BlendMode::Source,
            ..PixmapPaint::default()
        },
        Transform::from_scale(sx, sy),
        None,
    );
    dst
}

/// Scales an image to a new size. Large reductions are done in halving steps
/// so detail averages out instead of aliasing.
pub fn scale(src: &Pixmap, width: u32, height: u32, smooth: bool) -> Pixmap {
    let width = width.max(1);
    let height = height.max(1);
    if width == src.width() && height == src.height() {
        return src.clone();
    }
    if !smooth {
        return draw_scaled(src, width, height, FilterQuality::Nearest);
    }
    let mut current = src.clone();
    while current.width() / 2 >= width && current.height() / 2 >= height {
        current = draw_scaled(&current, current.width() / 2, current.height() / 2, FilterQuality::Bilinear);
    }
    draw_scaled(&current, width, height, FilterQuality::Bicubic)
}

/// Skews by the given angles (degrees, each in -89..=89). The canvas grows to fit;
/// uncovered pixels are transparent.
pub fn skew(src: &Pixmap, horizontal_deg: f32, vertical_deg: f32) -> Pixmap {
    let kx = horizontal_deg.clamp(-89.0, 89.0).to_radians().tan();
    let ky = vertical_deg.clamp(-89.0, 89.0).to_radians().tan();
    // Horizontal skew first, then vertical: x' = x + kx·y, y' = ky·x + (1 + kx·ky)·y.
    let t = Transform::from_row(1.0, ky, kx, 1.0 + kx * ky, 0.0, 0.0);
    let (w, h) = (src.width() as f32, src.height() as f32);
    let mut corners = [
        tiny_skia::Point::from_xy(0.0, 0.0),
        tiny_skia::Point::from_xy(w, 0.0),
        tiny_skia::Point::from_xy(0.0, h),
        tiny_skia::Point::from_xy(w, h),
    ];
    t.map_points(&mut corners);
    let min_x = corners.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
    let min_y = corners.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
    let max_x = corners.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
    let max_y = corners.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
    let new_w = ((max_x - min_x).round() as u32).clamp(1, super::document::MAX_DIMENSION);
    let new_h = ((max_y - min_y).round() as u32).clamp(1, super::document::MAX_DIMENSION);
    let mut dst = new_pixmap(new_w, new_h);
    dst.draw_pixmap(
        0,
        0,
        src.as_ref(),
        &PixmapPaint {
            quality: FilterQuality::Bilinear,
            ..PixmapPaint::default()
        },
        t.post_translate(-min_x, -min_y),
        None,
    );
    dst
}

/// Returns the part of `src` inside `region` (clamped to the image).
pub fn crop(src: &Pixmap, region: IntRect) -> Option<Pixmap> {
    let r = raster::clamp_to(region, src.width(), src.height())?;
    Some(raster::copy_region(src, r))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::color::Rgba;
    use crate::engine::raster::{filled, get_pixel, set_pixel};

    /// 3×2 image with a red pixel at the top-left and blue at the bottom-right.
    fn sample() -> Pixmap {
        let mut p = filled(3, 2, Rgba::WHITE);
        set_pixel(&mut p, 0, 0, Rgba::rgb(255, 0, 0));
        set_pixel(&mut p, 2, 1, Rgba::rgb(0, 0, 255));
        p
    }

    const RED: Rgba = Rgba::rgb(255, 0, 0);
    const BLUE: Rgba = Rgba::rgb(0, 0, 255);

    #[test]
    fn rotations() {
        let cw = rotate_cw(&sample());
        assert_eq!((cw.width(), cw.height()), (2, 3));
        assert_eq!(get_pixel(&cw, 1, 0), Some(RED));
        assert_eq!(get_pixel(&cw, 0, 2), Some(BLUE));

        let ccw = rotate_ccw(&sample());
        assert_eq!(get_pixel(&ccw, 0, 2), Some(RED));
        assert_eq!(get_pixel(&ccw, 1, 0), Some(BLUE));

        let r180 = rotate_180(&sample());
        assert_eq!(get_pixel(&r180, 2, 1), Some(RED));
        assert_eq!(get_pixel(&r180, 0, 0), Some(BLUE));

        let back = rotate_ccw(&rotate_cw(&sample()));
        assert_eq!(back.data(), sample().data());
    }

    #[test]
    fn flips() {
        let h = flip_horizontal(&sample());
        assert_eq!(get_pixel(&h, 2, 0), Some(RED));
        assert_eq!(get_pixel(&h, 0, 1), Some(BLUE));
        let v = flip_vertical(&sample());
        assert_eq!(get_pixel(&v, 0, 1), Some(RED));
        assert_eq!(get_pixel(&v, 2, 0), Some(BLUE));
    }

    #[test]
    fn scaling() {
        let up = scale(&sample(), 6, 4, false);
        assert_eq!((up.width(), up.height()), (6, 4));
        assert_eq!(get_pixel(&up, 1, 1), Some(RED));
        assert_eq!(get_pixel(&up, 5, 3), Some(BLUE));
        let down = scale(&filled(400, 200, Rgba::BLACK), 10, 5, true);
        assert_eq!((down.width(), down.height()), (10, 5));
        assert_eq!(get_pixel(&down, 5, 2), Some(Rgba::BLACK));
    }

    #[test]
    fn skew_grows_canvas() {
        let src = filled(100, 50, Rgba::BLACK);
        let out = skew(&src, 45.0, 0.0);
        assert_eq!((out.width(), out.height()), (150, 50));
        // Rows shift right in proportion to their distance from the top.
        assert_eq!(get_pixel(&out, 1, 0).unwrap().a, 255);
        assert_eq!(get_pixel(&out, 148, 49).unwrap().a, 255);
        assert_eq!(get_pixel(&out, 148, 0).unwrap().a, 0);
        assert_eq!(get_pixel(&out, 1, 49).unwrap().a, 0);
        let none = skew(&src, 0.0, 0.0);
        assert_eq!((none.width(), none.height()), (100, 50));
    }

    #[test]
    fn cropping() {
        let c = crop(&sample(), IntRect::from_xywh(1, 0, 5, 5).unwrap()).unwrap();
        assert_eq!((c.width(), c.height()), (2, 2));
        assert_eq!(get_pixel(&c, 1, 1), Some(BLUE));
        assert!(crop(&sample(), IntRect::from_xywh(10, 10, 2, 2).unwrap()).is_none());
    }
}
