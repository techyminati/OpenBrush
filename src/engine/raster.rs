// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Low-level helpers for working with premultiplied RGBA pixmaps and pixel rectangles.

use tiny_skia::{IntRect, Pixmap};

use super::color::Rgba;

/// Creates a transparent pixmap, clamping each side to at least one pixel.
pub fn new_pixmap(width: u32, height: u32) -> Pixmap {
    Pixmap::new(width.max(1), height.max(1)).expect("pixmap dimensions are valid")
}

/// Creates a pixmap filled with a single colour.
pub fn filled(width: u32, height: u32, color: Rgba) -> Pixmap {
    let mut p = new_pixmap(width, height);
    if color.a != 0 {
        p.fill(color.to_skia());
    }
    p
}

pub fn rect(x: i32, y: i32, w: u32, h: u32) -> Option<IntRect> {
    IntRect::from_xywh(x, y, w, h)
}

pub fn full_rect(p: &Pixmap) -> IntRect {
    IntRect::from_xywh(0, 0, p.width(), p.height()).expect("pixmap is non-empty")
}

/// Smallest rectangle covering both points (inclusive), grown by `pad` on every side.
pub fn rect_around(x0: f32, y0: f32, x1: f32, y1: f32, pad: f32) -> Option<IntRect> {
    let left = (x0.min(x1) - pad).floor() as i32;
    let top = (y0.min(y1) - pad).floor() as i32;
    let right = (x0.max(x1) + pad).ceil() as i32 + 1;
    let bottom = (y0.max(y1) + pad).ceil() as i32 + 1;
    IntRect::from_ltrb(left, top, right, bottom)
}

pub fn intersect(a: IntRect, b: IntRect) -> Option<IntRect> {
    IntRect::from_ltrb(
        a.left().max(b.left()),
        a.top().max(b.top()),
        a.right().min(b.right()),
        a.bottom().min(b.bottom()),
    )
}

pub fn union(a: IntRect, b: IntRect) -> IntRect {
    IntRect::from_ltrb(
        a.left().min(b.left()),
        a.top().min(b.top()),
        a.right().max(b.right()),
        a.bottom().max(b.bottom()),
    )
    .expect("union of valid rects is valid")
}

pub fn union_opt(a: Option<IntRect>, b: Option<IntRect>) -> Option<IntRect> {
    match (a, b) {
        (Some(a), Some(b)) => Some(union(a, b)),
        (a, None) => a,
        (None, b) => b,
    }
}

/// Clamps a rectangle to the bounds of a `width` × `height` image.
pub fn clamp_to(r: IntRect, width: u32, height: u32) -> Option<IntRect> {
    intersect(r, IntRect::from_xywh(0, 0, width, height)?)
}

/// Copies a region out of a pixmap. The region must lie within the pixmap.
pub fn copy_region(src: &Pixmap, r: IntRect) -> Pixmap {
    src.clone_rect(r).unwrap_or_else(|| new_pixmap(r.width(), r.height()))
}

/// Overwrites (no blending) the pixels of `dst` at `(x, y)` with `src`, clipping as needed.
pub fn replace_region(dst: &mut Pixmap, src: &Pixmap, x: i32, y: i32) {
    let dw = dst.width() as i32;
    let dh = dst.height() as i32;
    let sw = src.width() as i32;
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + sw).min(dw);
    let y1 = (y + src.height() as i32).min(dh);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let run = (x1 - x0) as usize;
    let src_px = src.pixels();
    let dst_px = dst.pixels_mut();
    for row in y0..y1 {
        let s = ((row - y) * sw + (x0 - x)) as usize;
        let d = (row * dw + x0) as usize;
        dst_px[d..d + run].copy_from_slice(&src_px[s..s + run]);
    }
}

/// Swaps the pixels of `patch` with the same-sized region of `dst` at `(x, y)`.
/// The region must lie entirely within `dst`.
pub fn swap_region(dst: &mut Pixmap, patch: &mut Pixmap, x: i32, y: i32) {
    let dw = dst.width() as usize;
    let pw = patch.width() as usize;
    let ph = patch.height() as usize;
    debug_assert!(x >= 0 && y >= 0);
    debug_assert!(x as usize + pw <= dw && y as usize + ph <= dst.height() as usize);
    let dst_px = dst.pixels_mut();
    let patch_px = patch.pixels_mut();
    for row in 0..ph {
        let d = (y as usize + row) * dw + x as usize;
        let p = row * pw;
        dst_px[d..d + pw].swap_with_slice(&mut patch_px[p..p + pw]);
    }
}

/// Sets every pixel inside `r` to `color` (no blending).
pub fn fill_region(dst: &mut Pixmap, r: IntRect, color: Rgba) {
    let Some(r) = clamp_to(r, dst.width(), dst.height()) else {
        return;
    };
    let px = color.premultiply();
    let w = dst.width() as usize;
    let pixels = dst.pixels_mut();
    for y in r.top()..r.bottom() {
        let start = y as usize * w + r.left() as usize;
        pixels[start..start + r.width() as usize].fill(px);
    }
}

pub fn get_pixel(p: &Pixmap, x: i32, y: i32) -> Option<Rgba> {
    if x < 0 || y < 0 {
        return None;
    }
    p.pixel(x as u32, y as u32).map(Rgba::from_premultiplied)
}

pub fn set_pixel(p: &mut Pixmap, x: i32, y: i32, color: Rgba) {
    if x < 0 || y < 0 || x >= p.width() as i32 || y >= p.height() as i32 {
        return;
    }
    let w = p.width() as usize;
    p.pixels_mut()[y as usize * w + x as usize] = color.premultiply();
}

/// Paints `color` at `alpha` coverage, keeping whichever coverage is greater.
/// Used by textured brushes so repeated dabs don't build up within one stroke.
pub fn max_blend_pixel(p: &mut Pixmap, x: i32, y: i32, color: Rgba, alpha: u8) {
    if x < 0 || y < 0 || x >= p.width() as i32 || y >= p.height() as i32 || alpha == 0 {
        return;
    }
    let w = p.width() as usize;
    let idx = y as usize * w + x as usize;
    let current = p.pixels()[idx];
    if current.alpha() >= alpha {
        return;
    }
    p.pixels_mut()[idx] = color.with_alpha(alpha).premultiply();
}

/// True when every pixel is fully transparent.
pub fn is_blank(p: &Pixmap) -> bool {
    p.pixels().iter().all(|px| px.alpha() == 0)
}

/// Bounding box of all non-transparent pixels.
pub fn content_bounds(p: &Pixmap) -> Option<IntRect> {
    let w = p.width() as i32;
    let (mut l, mut t, mut r, mut b) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for (i, px) in p.pixels().iter().enumerate() {
        if px.alpha() != 0 {
            let x = i as i32 % w;
            let y = i as i32 / w;
            l = l.min(x);
            t = t.min(y);
            r = r.max(x);
            b = b.max(y);
        }
    }
    if l > r {
        None
    } else {
        IntRect::from_ltrb(l, t, r + 1, b + 1)
    }
}

/// Converts premultiplied pixels to straight-alpha RGBA bytes.
pub fn to_rgba8(p: &Pixmap) -> Vec<u8> {
    p.clone().take_demultiplied()
}

/// Builds a pixmap from straight-alpha RGBA bytes.
pub fn from_rgba8(width: u32, height: u32, rgba: &[u8]) -> Option<Pixmap> {
    let mut p = Pixmap::new(width, height)?;
    if rgba.len() != (width * height * 4) as usize {
        return None;
    }
    for (dst, src) in p.pixels_mut().iter_mut().zip(rgba.chunks_exact(4)) {
        *dst = Rgba::new(src[0], src[1], src[2], src[3]).premultiply();
    }
    Some(p)
}

/// Composites `p` over an opaque background colour, producing an opaque image.
pub fn flatten_onto(p: &Pixmap, background: Rgba) -> Pixmap {
    let mut out = filled(p.width(), p.height(), background);
    out.draw_pixmap(
        0,
        0,
        p.as_ref(),
        &tiny_skia::PixmapPaint::default(),
        tiny_skia::Transform::identity(),
        None,
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_region_clips() {
        let mut dst = filled(4, 4, Rgba::WHITE);
        let src = filled(3, 3, Rgba::BLACK);
        replace_region(&mut dst, &src, 2, 2);
        assert_eq!(get_pixel(&dst, 1, 1), Some(Rgba::WHITE));
        assert_eq!(get_pixel(&dst, 2, 2), Some(Rgba::BLACK));
        assert_eq!(get_pixel(&dst, 3, 3), Some(Rgba::BLACK));
        replace_region(&mut dst, &src, -2, -2);
        assert_eq!(get_pixel(&dst, 0, 0), Some(Rgba::BLACK));
        assert_eq!(get_pixel(&dst, 1, 0), Some(Rgba::WHITE));
    }

    #[test]
    fn swap_region_is_reversible() {
        let mut dst = filled(4, 4, Rgba::WHITE);
        let mut patch = filled(2, 2, Rgba::BLACK);
        swap_region(&mut dst, &mut patch, 1, 1);
        assert_eq!(get_pixel(&dst, 1, 1), Some(Rgba::BLACK));
        assert_eq!(get_pixel(&patch, 0, 0), Some(Rgba::WHITE));
        swap_region(&mut dst, &mut patch, 1, 1);
        assert_eq!(get_pixel(&dst, 1, 1), Some(Rgba::WHITE));
        assert_eq!(get_pixel(&patch, 1, 1), Some(Rgba::BLACK));
    }

    #[test]
    fn content_bounds_finds_pixels() {
        let mut p = new_pixmap(10, 10);
        assert_eq!(content_bounds(&p), None);
        set_pixel(&mut p, 2, 3, Rgba::BLACK);
        set_pixel(&mut p, 6, 8, Rgba::BLACK);
        assert_eq!(content_bounds(&p), IntRect::from_ltrb(2, 3, 7, 9));
    }

    #[test]
    fn rgba8_round_trip() {
        let bytes = [255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 0, 0, 1, 2, 3, 255];
        let p = from_rgba8(2, 2, &bytes).unwrap();
        let back = to_rgba8(&p);
        assert_eq!(&back[0..4], &bytes[0..4]);
        assert_eq!(&back[12..16], &bytes[12..16]);
        assert_eq!(back[7], 128);
    }

    #[test]
    fn rect_helpers() {
        let a = IntRect::from_xywh(0, 0, 10, 10).unwrap();
        let b = IntRect::from_xywh(5, 5, 10, 10).unwrap();
        assert_eq!(intersect(a, b), IntRect::from_xywh(5, 5, 5, 5));
        assert_eq!(union(a, b), IntRect::from_xywh(0, 0, 15, 15).unwrap());
        assert_eq!(clamp_to(b, 8, 8), IntRect::from_xywh(5, 5, 3, 3));
        assert_eq!(intersect(a, IntRect::from_xywh(20, 20, 1, 1).unwrap()), None);
    }
}
