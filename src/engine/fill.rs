// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Flood fill ("Fill with color").

use tiny_skia::{IntRect, Pixmap, PremultipliedColorU8};

use super::color::Rgba;

/// Tolerance used by the Fill tool. A little slack lets the fill reach into the
/// anti-aliased edges of brush strokes instead of leaving a halo.
pub const DEFAULT_TOLERANCE: u8 = 32;

fn diff(a: PremultipliedColorU8, b: PremultipliedColorU8) -> u8 {
    a.red()
        .abs_diff(b.red())
        .max(a.green().abs_diff(b.green()))
        .max(a.blue().abs_diff(b.blue()))
        .max(a.alpha().abs_diff(b.alpha()))
}

/// Region connected to `(x, y)` whose pixels are within `tolerance` of the start pixel,
/// as a per-pixel mask (`true` = inside) plus its bounding box.
pub fn region_mask(p: &Pixmap, x: i32, y: i32, tolerance: u8) -> Option<(Vec<bool>, IntRect)> {
    let w = p.width() as i32;
    let h = p.height() as i32;
    if x < 0 || y < 0 || x >= w || y >= h {
        return None;
    }
    let pixels = p.pixels();
    let target = pixels[(y * w + x) as usize];
    let mut inside = vec![false; (w * h) as usize];
    let matches = |i: usize, inside: &Vec<bool>| !inside[i] && diff(pixels[i], target) <= tolerance;

    let (mut left, mut top, mut right, mut bottom) = (x, y, x, y);
    let mut stack = vec![(x, y)];
    while let Some((sx, sy)) = stack.pop() {
        let row = (sy * w) as usize;
        if !matches(row + sx as usize, &inside) {
            continue;
        }
        let mut x0 = sx;
        while x0 > 0 && matches(row + (x0 - 1) as usize, &inside) {
            x0 -= 1;
        }
        let mut x1 = sx;
        while x1 + 1 < w && matches(row + (x1 + 1) as usize, &inside) {
            x1 += 1;
        }
        for xi in x0..=x1 {
            inside[row + xi as usize] = true;
        }
        left = left.min(x0);
        right = right.max(x1);
        top = top.min(sy);
        bottom = bottom.max(sy);
        for ny in [sy - 1, sy + 1] {
            if ny < 0 || ny >= h {
                continue;
            }
            let nrow = (ny * w) as usize;
            let mut xi = x0;
            while xi <= x1 {
                if matches(nrow + xi as usize, &inside) {
                    stack.push((xi, ny));
                    // Skip the rest of this run; it will be expanded from the seed.
                    while xi <= x1 && matches(nrow + xi as usize, &inside) {
                        xi += 1;
                    }
                } else {
                    xi += 1;
                }
            }
        }
    }
    let bounds = IntRect::from_ltrb(left, top, right + 1, bottom + 1)?;
    Some((inside, bounds))
}

/// Replaces the connected region around `(x, y)` with `color`.
/// Returns the bounding box of changed pixels, or `None` if nothing changed.
pub fn flood_fill(p: &mut Pixmap, x: i32, y: i32, color: Rgba, tolerance: u8) -> Option<IntRect> {
    let fill = color.premultiply();
    let w = p.width() as i32;
    if x < 0 || y < 0 || x >= w || y >= p.height() as i32 {
        return None;
    }
    if p.pixels()[(y * w + x) as usize] == fill && tolerance == 0 {
        return None;
    }
    let (inside, bounds) = region_mask(p, x, y, tolerance)?;
    for (px, &hit) in p.pixels_mut().iter_mut().zip(&inside) {
        if hit {
            *px = fill;
        }
    }
    Some(bounds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::raster::{fill_region, filled, get_pixel};

    #[test]
    fn fills_enclosed_area_only() {
        let mut p = filled(10, 10, Rgba::WHITE);
        // A black ring from (2,2) to (7,7).
        fill_region(&mut p, IntRect::from_xywh(2, 2, 6, 1).unwrap(), Rgba::BLACK);
        fill_region(&mut p, IntRect::from_xywh(2, 7, 6, 1).unwrap(), Rgba::BLACK);
        fill_region(&mut p, IntRect::from_xywh(2, 2, 1, 6).unwrap(), Rgba::BLACK);
        fill_region(&mut p, IntRect::from_xywh(7, 2, 1, 6).unwrap(), Rgba::BLACK);
        let red = Rgba::rgb(255, 0, 0);
        let changed = flood_fill(&mut p, 4, 4, red, 0).unwrap();
        assert_eq!(changed, IntRect::from_xywh(3, 3, 4, 4).unwrap());
        assert_eq!(get_pixel(&p, 4, 4), Some(red));
        assert_eq!(get_pixel(&p, 2, 2), Some(Rgba::BLACK));
        assert_eq!(get_pixel(&p, 0, 0), Some(Rgba::WHITE));
    }

    #[test]
    fn fills_concave_regions() {
        // A "U" shaped white region: fill must flow around the black bar.
        let mut p = filled(5, 5, Rgba::WHITE);
        fill_region(&mut p, IntRect::from_xywh(2, 0, 1, 4).unwrap(), Rgba::BLACK);
        flood_fill(&mut p, 0, 0, Rgba::BLACK, 0).unwrap();
        assert_eq!(get_pixel(&p, 4, 0), Some(Rgba::BLACK));
        assert!(crate::engine::raster::to_rgba8(&p).chunks(4).all(|c| c[0] == 0));
    }

    #[test]
    fn same_colour_is_noop() {
        let mut p = filled(3, 3, Rgba::WHITE);
        assert_eq!(flood_fill(&mut p, 1, 1, Rgba::WHITE, 0), None);
        assert_eq!(flood_fill(&mut p, 5, 5, Rgba::BLACK, 0), None);
    }

    #[test]
    fn tolerance_reaches_similar_colours() {
        let mut p = filled(3, 1, Rgba::WHITE);
        crate::engine::raster::set_pixel(&mut p, 1, 0, Rgba::rgb(250, 250, 250));
        crate::engine::raster::set_pixel(&mut p, 2, 0, Rgba::rgb(100, 100, 100));
        flood_fill(&mut p, 0, 0, Rgba::BLACK, DEFAULT_TOLERANCE);
        assert_eq!(get_pixel(&p, 1, 0), Some(Rgba::BLACK));
        assert_eq!(get_pixel(&p, 2, 0), Some(Rgba::rgb(100, 100, 100)));
    }

    #[test]
    fn fills_transparent_area() {
        let mut p = crate::engine::raster::new_pixmap(4, 4);
        flood_fill(&mut p, 0, 0, Rgba::rgb(0, 0, 255), 0).unwrap();
        assert_eq!(get_pixel(&p, 3, 3), Some(Rgba::rgb(0, 0, 255)));
    }
}
