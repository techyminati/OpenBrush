// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Rectangular and free-form selections, and the floating pixels they carry when moved.

use tiny_skia::{FillRule, FilterQuality, IntRect, Mask, PathBuilder, Pixmap, PixmapPaint, Point, Rect, Transform};

use super::color::Rgba;
use super::raster;

/// Pixels lifted off the canvas, positioned (and possibly scaled) in document space.
#[derive(Clone)]
pub struct Floating {
    pub pixels: Pixmap,
    pub rect: Rect,
}

#[derive(Clone)]
pub struct Selection {
    /// The area of the layer that was selected.
    pub origin: IntRect,
    /// Coverage within `origin` for non-rectangular selections.
    pub mask: Option<Mask>,
    /// Set once the selection has been moved, resized, transformed or pasted.
    pub floating: Option<Floating>,
}

impl Selection {
    pub fn rect(r: IntRect) -> Self {
        Self {
            origin: r,
            mask: None,
            floating: None,
        }
    }

    pub fn all(width: u32, height: u32) -> Self {
        Self::rect(IntRect::from_xywh(0, 0, width, height).expect("document is non-empty"))
    }

    /// A free-form (lasso) selection, clipped to the document.
    pub fn freeform(points: &[Point], width: u32, height: u32) -> Option<Self> {
        if points.len() < 3 {
            return None;
        }
        let mut pb = PathBuilder::new();
        pb.move_to(points[0].x, points[0].y);
        for p in &points[1..] {
            pb.line_to(p.x, p.y);
        }
        pb.close();
        let path = pb.finish()?;
        let b = path.bounds();
        let origin = raster::rect_around(b.left(), b.top(), b.right(), b.bottom(), 0.0)
            .and_then(|r| raster::clamp_to(r, width, height))?;
        let mut mask = Mask::new(origin.width(), origin.height())?;
        mask.fill_path(
            &path,
            FillRule::Winding,
            false,
            Transform::from_translate(-origin.x() as f32, -origin.y() as f32),
        );
        if mask.data().iter().all(|&m| m == 0) {
            return None;
        }
        Some(Self {
            origin,
            mask: Some(mask),
            floating: None,
        })
    }

    /// A pasted image, floating at `(x, y)`.
    pub fn pasted(pixels: Pixmap, x: i32, y: i32) -> Self {
        let origin = IntRect::from_xywh(x, y, pixels.width(), pixels.height()).expect("pixmap is non-empty");
        let rect = Rect::from_xywh(x as f32, y as f32, pixels.width() as f32, pixels.height() as f32).expect("valid");
        Self {
            origin,
            mask: None,
            floating: Some(Floating { pixels, rect }),
        }
    }

    /// Selects everything that is *not* currently selected.
    pub fn inverted(&self, width: u32, height: u32) -> Option<Self> {
        let mut mask = Mask::new(width, height)?;
        mask.data_mut().fill(255);
        let w = width as usize;
        let ow = self.origin.width() as usize;
        for y in self.origin.top().max(0)..self.origin.bottom().min(height as i32) {
            for x in self.origin.left().max(0)..self.origin.right().min(width as i32) {
                let local = (y - self.origin.top()) as usize * ow + (x - self.origin.left()) as usize;
                let selected = self.mask.as_ref().map_or(255, |m| m.data()[local]);
                mask.data_mut()[y as usize * w + x as usize] = 255 - selected;
            }
        }
        if mask.data().iter().all(|&m| m == 0) {
            return None;
        }
        Some(Self {
            origin: IntRect::from_xywh(0, 0, width, height)?,
            mask: Some(mask),
            floating: None,
        })
    }

    pub fn is_floating(&self) -> bool {
        self.floating.is_some()
    }

    /// Where the selection currently is, in document coordinates.
    pub fn bounds(&self) -> Rect {
        match &self.floating {
            Some(f) => f.rect,
            None => self.origin.to_rect(),
        }
    }

    /// Selected pixels of `layer`, with unselected pixels transparent.
    pub fn extract(&self, layer: &Pixmap) -> Option<Pixmap> {
        if let Some(f) = &self.floating {
            return Some(self.render_floating(f, None));
        }
        let r = raster::clamp_to(self.origin, layer.width(), layer.height())?;
        let mut out = raster::new_pixmap(self.origin.width(), self.origin.height());
        let copy = raster::copy_region(layer, r);
        raster::replace_region(&mut out, &copy, r.x() - self.origin.x(), r.y() - self.origin.y());
        if let Some(mask) = &self.mask {
            out.apply_mask(mask);
        }
        Some(out)
    }

    /// Makes the selected pixels of `layer` transparent.
    pub fn clear(&self, layer: &mut Pixmap) {
        let Some(r) = raster::clamp_to(self.origin, layer.width(), layer.height()) else {
            return;
        };
        let lw = layer.width() as usize;
        let ow = self.origin.width() as usize;
        let pixels = layer.pixels_mut();
        for y in r.top()..r.bottom() {
            for x in r.left()..r.right() {
                let idx = y as usize * lw + x as usize;
                let keep = match &self.mask {
                    None => 0,
                    Some(m) => {
                        let local = (y - self.origin.top()) as usize * ow + (x - self.origin.left()) as usize;
                        255 - m.data()[local] as u16
                    }
                };
                if keep == 0 {
                    pixels[idx] = tiny_skia::PremultipliedColorU8::TRANSPARENT;
                } else if keep < 255 {
                    let p = pixels[idx];
                    let s = |c: u8| ((c as u16 * keep + 127) / 255) as u8;
                    pixels[idx] = tiny_skia::PremultipliedColorU8::from_rgba(s(p.red()), s(p.green()), s(p.blue()), s(p.alpha()))
                        .unwrap_or(tiny_skia::PremultipliedColorU8::TRANSPARENT);
                }
            }
        }
    }

    /// Lifts the selected pixels so they can be moved. Unless `duplicate` is set,
    /// the area they came from becomes transparent.
    pub fn lift(&mut self, layer: &mut Pixmap, duplicate: bool) {
        if self.floating.is_some() {
            return;
        }
        let Some(pixels) = self.extract(layer) else {
            return;
        };
        if !duplicate {
            self.clear(layer);
        }
        self.floating = Some(Floating {
            pixels,
            rect: self.origin.to_rect(),
        });
    }

    /// Floating pixels as they will be drawn, optionally keying out `transparent` (Paint's
    /// "Transparent selection" treats the secondary colour as see-through).
    fn render_floating(&self, f: &Floating, transparent: Option<Rgba>) -> Pixmap {
        let mut pixels = f.pixels.clone();
        if let Some(key) = transparent {
            let key = key.premultiply();
            for px in pixels.pixels_mut() {
                if *px == key {
                    *px = tiny_skia::PremultipliedColorU8::TRANSPARENT;
                }
            }
        }
        pixels
    }

    /// Draws the floating pixels onto `target`, whose top-left is at `offset` in document space.
    pub fn draw_floating(&self, target: &mut Pixmap, offset: (i32, i32), transparent: Option<Rgba>) {
        let Some(f) = &self.floating else {
            return;
        };
        let pixels = self.render_floating(f, transparent);
        let sx = f.rect.width() / pixels.width() as f32;
        let sy = f.rect.height() / pixels.height() as f32;
        let scaled = (sx - 1.0).abs() > 1e-4 || (sy - 1.0).abs() > 1e-4;
        let fractional = f.rect.x().fract() != 0.0 || f.rect.y().fract() != 0.0;
        let paint = PixmapPaint {
            quality: if scaled || fractional { FilterQuality::Bilinear } else { FilterQuality::Nearest },
            ..PixmapPaint::default()
        };
        let t = Transform::from_row(
            sx,
            0.0,
            0.0,
            sy,
            f.rect.x() - offset.0 as f32,
            f.rect.y() - offset.1 as f32,
        );
        target.draw_pixmap(0, 0, pixels.as_ref(), &paint, t, None);
    }

    /// The document area the floating pixels currently cover.
    pub fn floating_area(&self) -> Option<IntRect> {
        let r = self.floating.as_ref()?.rect;
        raster::rect_around(r.left(), r.top(), r.right() - 1.0, r.bottom() - 1.0, 1.0)
    }

    /// Replaces the floating pixels, keeping the top-left corner fixed (used by rotate/flip/resize).
    pub fn replace_floating(&mut self, pixels: Pixmap) {
        let (x, y) = match &self.floating {
            Some(f) => (f.rect.x(), f.rect.y()),
            None => (self.origin.x() as f32, self.origin.y() as f32),
        };
        let rect = Rect::from_xywh(x, y, pixels.width() as f32, pixels.height() as f32).expect("valid");
        self.floating = Some(Floating { pixels, rect });
    }

    /// Whether a document point lies inside the selection's current bounds.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        let b = self.bounds();
        x >= b.left() && x < b.right() && y >= b.top() && y < b.bottom()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::raster::{filled, get_pixel, set_pixel};

    fn layer() -> Pixmap {
        let mut p = filled(10, 10, Rgba::WHITE);
        set_pixel(&mut p, 2, 2, Rgba::BLACK);
        p
    }

    #[test]
    fn move_rect_selection() {
        let mut l = layer();
        let mut s = Selection::rect(IntRect::from_xywh(1, 1, 3, 3).unwrap());
        s.lift(&mut l, false);
        assert_eq!(get_pixel(&l, 2, 2).unwrap().a, 0, "source is cleared");
        s.floating.as_mut().unwrap().rect = Rect::from_xywh(5.0, 5.0, 3.0, 3.0).unwrap();
        s.draw_floating(&mut l, (0, 0), None);
        assert_eq!(get_pixel(&l, 6, 6), Some(Rgba::BLACK));
        assert_eq!(get_pixel(&l, 5, 5), Some(Rgba::WHITE));
    }

    #[test]
    fn duplicate_keeps_source() {
        let mut l = layer();
        let mut s = Selection::rect(IntRect::from_xywh(1, 1, 3, 3).unwrap());
        s.lift(&mut l, true);
        assert_eq!(get_pixel(&l, 2, 2), Some(Rgba::BLACK));
    }

    #[test]
    fn transparent_selection_keys_out_colour() {
        let mut l = layer();
        let mut s = Selection::rect(IntRect::from_xywh(1, 1, 3, 3).unwrap());
        s.lift(&mut l, true);
        let mut target = filled(10, 10, Rgba::rgb(255, 0, 0));
        s.draw_floating(&mut target, (0, 0), Some(Rgba::WHITE));
        assert_eq!(get_pixel(&target, 1, 1), Some(Rgba::rgb(255, 0, 0)), "white keyed out");
        assert_eq!(get_pixel(&target, 2, 2), Some(Rgba::BLACK));
    }

    #[test]
    fn freeform_mask_limits_selection() {
        let mut l = filled(10, 10, Rgba::BLACK);
        let tri = [Point::from_xy(0.0, 0.0), Point::from_xy(10.0, 0.0), Point::from_xy(0.0, 10.0)];
        let s = Selection::freeform(&tri, 10, 10).unwrap();
        s.clear(&mut l);
        assert_eq!(get_pixel(&l, 1, 1).unwrap().a, 0);
        assert_eq!(get_pixel(&l, 9, 9), Some(Rgba::BLACK));
        let extracted = s.extract(&filled(10, 10, Rgba::BLACK)).unwrap();
        assert_eq!(get_pixel(&extracted, 9, 9).unwrap().a, 0);
        assert_eq!(get_pixel(&extracted, 1, 1), Some(Rgba::BLACK));
    }

    #[test]
    fn invert_selects_the_rest() {
        let s = Selection::rect(IntRect::from_xywh(0, 0, 5, 10).unwrap());
        let inv = s.inverted(10, 10).unwrap();
        let mut l = filled(10, 10, Rgba::BLACK);
        inv.clear(&mut l);
        assert_eq!(get_pixel(&l, 2, 2), Some(Rgba::BLACK));
        assert_eq!(get_pixel(&l, 7, 2).unwrap().a, 0);
        assert!(Selection::all(10, 10).inverted(10, 10).is_none());
    }

    #[test]
    fn scaled_floating_draws_larger() {
        let mut s = Selection::pasted(filled(2, 2, Rgba::BLACK), 0, 0);
        s.floating.as_mut().unwrap().rect = Rect::from_xywh(0.0, 0.0, 6.0, 6.0).unwrap();
        let mut target = filled(10, 10, Rgba::WHITE);
        s.draw_floating(&mut target, (0, 0), None);
        assert_eq!(get_pixel(&target, 5, 5).unwrap().r, 0);
        assert_eq!(get_pixel(&target, 8, 8), Some(Rgba::WHITE));
    }

    #[test]
    fn selection_partly_outside_canvas() {
        let l = filled(10, 10, Rgba::BLACK);
        let s = Selection::rect(IntRect::from_xywh(8, 8, 4, 4).unwrap());
        let e = s.extract(&l).unwrap();
        assert_eq!((e.width(), e.height()), (4, 4));
        assert_eq!(get_pixel(&e, 0, 0), Some(Rgba::BLACK));
        assert_eq!(get_pixel(&e, 3, 3).unwrap().a, 0);
    }
}
