// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Freehand stroke rendering for the pencil, eraser and the nine brushes.
//!
//! A stroke is drawn into a transparent *draft* pixmap the size of the document.
//! When the stroke ends, the draft is composited onto the layer with
//! [`Stroke::composite_paint`]. Drawing into a draft first means translucent brushes
//! (marker, watercolour) don't darken where a single stroke overlaps itself.

use tiny_skia::{FillRule, IntRect, LineCap, LineJoin, Paint, PathBuilder, Pixmap, PixmapPaint, Point, Transform};

use super::color::Rgba;
use super::raster;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum BrushKind {
    Brush,
    CalligraphyBrush,
    CalligraphyPen,
    Airbrush,
    OilBrush,
    Crayon,
    Marker,
    NaturalPencil,
    Watercolor,
}

impl BrushKind {
    pub const ALL: [BrushKind; 9] = [
        BrushKind::Brush,
        BrushKind::CalligraphyBrush,
        BrushKind::CalligraphyPen,
        BrushKind::Airbrush,
        BrushKind::OilBrush,
        BrushKind::Crayon,
        BrushKind::Marker,
        BrushKind::NaturalPencil,
        BrushKind::Watercolor,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BrushKind::Brush => "Brush",
            BrushKind::CalligraphyBrush => "Calligraphy brush",
            BrushKind::CalligraphyPen => "Calligraphy pen",
            BrushKind::Airbrush => "Airbrush",
            BrushKind::OilBrush => "Oil brush",
            BrushKind::Crayon => "Crayon",
            BrushKind::Marker => "Marker",
            BrushKind::NaturalPencil => "Natural pencil",
            BrushKind::Watercolor => "Watercolor brush",
        }
    }
}

/// What a stroke does.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum StrokeKind {
    Pencil,
    Eraser,
    Brush(BrushKind),
}

/// Small deterministic PRNG (xorshift64*), so strokes are reproducible in tests.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }
}

/// Stable per-pixel hash in `[0, 1)`; gives textured brushes a consistent "paper grain".
pub fn grain(x: i32, y: i32, salt: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (y as u32).wrapping_mul(0xD816_3841) ^ salt.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65536.0
}

/// Smooth value noise in `[0, 1)` with features roughly `scale` pixels across.
pub fn value_noise(x: f32, y: f32, scale: f32, salt: u32) -> f32 {
    let fx = x / scale;
    let fy = y / scale;
    let x0 = fx.floor();
    let y0 = fy.floor();
    let tx = fx - x0;
    let ty = fy - y0;
    let sx = tx * tx * (3.0 - 2.0 * tx);
    let sy = ty * ty * (3.0 - 2.0 * ty);
    let (ix, iy) = (x0 as i32, y0 as i32);
    let a = grain(ix, iy, salt);
    let b = grain(ix + 1, iy, salt);
    let c = grain(ix, iy + 1, salt);
    let d = grain(ix + 1, iy + 1, salt);
    let top = a + (b - a) * sx;
    let bottom = c + (d - c) * sx;
    top + (bottom - top) * sy
}

fn solid_paint(color: Rgba) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(color.to_skia());
    paint.anti_alias = true;
    paint
}

/// Bounding box of a segment grown by `pad`, clamped to the draft.
fn segment_bounds(draft: &Pixmap, a: Point, b: Point, pad: f32) -> Option<IntRect> {
    let r = raster::rect_around(a.x, a.y, b.x, b.y, pad)?;
    raster::clamp_to(r, draft.width(), draft.height())
}

/// Convex hull (monotone chain) of a point set; used to sweep a square marker tip.
fn convex_hull(mut pts: Vec<Point>) -> Vec<Point> {
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    pts.dedup_by(|a, b| (a.x - b.x).abs() < 1e-4 && (a.y - b.y).abs() < 1e-4);
    if pts.len() < 3 {
        return pts;
    }
    let cross = |o: Point, a: Point, b: Point| (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    let mut hull: Vec<Point> = Vec::with_capacity(pts.len() * 2);
    // Lower hull, then upper hull; the upper pass may not pop into the lower one.
    for &p in &pts {
        while hull.len() >= 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
            hull.pop();
        }
        hull.push(p);
    }
    let lower_len = hull.len() + 1;
    for &p in pts.iter().rev().skip(1) {
        while hull.len() >= lower_len && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
            hull.pop();
        }
        hull.push(p);
    }
    hull.pop();
    hull
}

fn fill_polygon(draft: &mut Pixmap, pts: &[Point], color: Rgba) {
    let mut pb = PathBuilder::new();
    let Some((first, rest)) = pts.split_first() else {
        return;
    };
    pb.move_to(first.x, first.y);
    for p in rest {
        pb.line_to(p.x, p.y);
    }
    pb.close();
    if let Some(path) = pb.finish() {
        draft.fill_path(&path, &solid_paint(color), FillRule::Winding, Transform::identity(), None);
    }
}

/// Bresenham line of `size`×`size` aliased squares, as Paint's pencil and eraser draw.
fn square_line(draft: &mut Pixmap, a: Point, b: Point, size: u32, color: Rgba) {
    let size = size.max(1) as i32;
    let offset = (size - 1) / 2;
    let (mut x0, mut y0) = (a.x.floor() as i32, a.y.floor() as i32);
    let (x1, y1) = (b.x.floor() as i32, b.y.floor() as i32);
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        if let Some(r) = IntRect::from_xywh(x0 - offset, y0 - offset, size as u32, size as u32) {
            raster::fill_region(draft, r, color);
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// Renders a capsule (a segment with round ends) pixel by pixel. `coverage` receives the
/// pixel position, the distance from the centre line as a fraction of `radius` (0..1), and
/// the signed offset across the stroke (-1..1); it returns opacity (0..1) and a colour.
fn capsule(
    draft: &mut Pixmap,
    a: Point,
    b: Point,
    radius: f32,
    mut coverage: impl FnMut(i32, i32, f32, f32) -> (f32, Rgba),
) -> Option<IntRect> {
    let bounds = segment_bounds(draft, a, b, radius + 1.0)?;
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len_sq = dx * dx + dy * dy;
    let len = len_sq.sqrt();
    let (nx, ny) = if len > 1e-3 { (-dy / len, dx / len) } else { (0.0, 1.0) };
    for y in bounds.top()..bounds.bottom() {
        for x in bounds.left()..bounds.right() {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let t = if len_sq > 1e-6 {
                (((px - a.x) * dx + (py - a.y) * dy) / len_sq).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let cx = a.x + dx * t;
            let cy = a.y + dy * t;
            let dist = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
            if dist > radius + 0.5 {
                continue;
            }
            let edge = (radius + 0.5 - dist).clamp(0.0, 1.0);
            let across = (((px - a.x) * nx + (py - a.y) * ny) / radius.max(0.5)).clamp(-1.0, 1.0);
            let (alpha, color) = coverage(x, y, (dist / radius.max(0.5)).min(1.0), across);
            let alpha = (alpha * edge).clamp(0.0, 1.0);
            raster::max_blend_pixel(draft, x, y, color, (alpha * 255.0).round() as u8);
        }
    }
    Some(bounds)
}

fn shade(color: Rgba, amount: f32) -> Rgba {
    let f = |c: u8| {
        let c = c as f32;
        let v = if amount >= 0.0 { c + (255.0 - c) * amount } else { c * (1.0 + amount) };
        v.round().clamp(0.0, 255.0) as u8
    };
    Rgba::new(f(color.r), f(color.g), f(color.b), color.a)
}

struct Bristle {
    /// Position across the stroke, -1..1.
    offset: f32,
    alpha: f32,
    shade: f32,
}

/// An in-progress freehand stroke.
pub struct Stroke {
    kind: StrokeKind,
    color: Rgba,
    size: f32,
    last: Option<Point>,
    travelled: f32,
    rng: Rng,
    bristles: Vec<Bristle>,
}

impl Stroke {
    pub fn new(kind: StrokeKind, color: Rgba, size: f32, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let mut bristles = Vec::new();
        if kind == StrokeKind::Brush(BrushKind::OilBrush) {
            let count = (size / 2.0).clamp(5.0, 24.0) as usize;
            for i in 0..count {
                let base = (i as f32 + 0.5) / count as f32 * 2.0 - 1.0;
                bristles.push(Bristle {
                    offset: base + rng.range(-0.4, 0.4) / count as f32,
                    alpha: rng.range(0.88, 1.0),
                    shade: rng.range(-0.08, 0.08),
                });
            }
        }
        Self {
            kind,
            color: color.with_alpha(255),
            size: size.max(1.0),
            last: None,
            travelled: 0.0,
            rng,
            bristles,
        }
    }

    pub fn kind(&self) -> StrokeKind {
        self.kind
    }

    pub fn is_eraser(&self) -> bool {
        self.kind == StrokeKind::Eraser
    }

    /// Whether the stroke keeps painting while the pointer is held still.
    pub fn is_continuous(&self) -> bool {
        self.kind == StrokeKind::Brush(BrushKind::Airbrush)
    }

    /// How the finished draft is combined with the layer.
    pub fn composite_paint(&self) -> PixmapPaint {
        let opacity = match self.kind {
            StrokeKind::Brush(BrushKind::Marker) => 0.55,
            StrokeKind::Brush(BrushKind::Watercolor) => 0.8,
            _ => 1.0,
        };
        PixmapPaint {
            opacity,
            blend_mode: if self.is_eraser() {
                tiny_skia::BlendMode::DestinationOut
            } else {
                tiny_skia::BlendMode::SourceOver
            },
            quality: tiny_skia::FilterQuality::Nearest,
        }
    }

    /// Adds a point to the stroke. Returns the area of the draft that changed.
    pub fn add_point(&mut self, draft: &mut Pixmap, x: f32, y: f32) -> Option<IntRect> {
        let p = Point::from_xy(x, y);
        let from = self.last.unwrap_or(p);
        self.last = Some(p);
        let dirty = self.segment(draft, from, p);
        self.travelled += ((p.x - from.x).powi(2) + (p.y - from.y).powi(2)).sqrt();
        dirty
    }

    /// Advances time for continuous brushes (the airbrush keeps spraying in place).
    pub fn tick(&mut self, draft: &mut Pixmap, seconds: f32) -> Option<IntRect> {
        if !self.is_continuous() {
            return None;
        }
        let p = self.last?;
        let radius = self.airbrush_radius();
        let dots = (radius * 60.0 * seconds).ceil() as usize;
        self.spray(draft, p, radius, dots)
    }

    fn airbrush_radius(&self) -> f32 {
        self.size.max(3.0)
    }

    fn spray(&mut self, draft: &mut Pixmap, at: Point, radius: f32, dots: usize) -> Option<IntRect> {
        for _ in 0..dots {
            let angle = self.rng.range(0.0, std::f32::consts::TAU);
            let dist = radius * self.rng.next_f32().sqrt();
            let x = (at.x + angle.cos() * dist).floor() as i32;
            let y = (at.y + angle.sin() * dist).floor() as i32;
            raster::max_blend_pixel(draft, x, y, self.color, 255);
        }
        segment_bounds(draft, at, at, radius + 1.0)
    }

    fn segment(&mut self, draft: &mut Pixmap, a: Point, b: Point) -> Option<IntRect> {
        let color = self.color;
        let size = self.size;
        let radius = size / 2.0;
        match self.kind {
            StrokeKind::Pencil | StrokeKind::Eraser => {
                square_line(draft, a, b, size.round() as u32, color);
                segment_bounds(draft, a, b, size)
            }
            StrokeKind::Brush(kind) => match kind {
                BrushKind::Brush => {
                    let mut pb = PathBuilder::new();
                    pb.move_to(a.x, a.y);
                    pb.line_to(b.x + 0.01, b.y);
                    let path = pb.finish()?;
                    let stroke = tiny_skia::Stroke {
                        width: size,
                        line_cap: LineCap::Round,
                        line_join: LineJoin::Round,
                        ..Default::default()
                    };
                    draft.stroke_path(&path, &solid_paint(color), &stroke, Transform::identity(), None);
                    segment_bounds(draft, a, b, radius + 2.0)
                }
                BrushKind::CalligraphyBrush | BrushKind::CalligraphyPen => {
                    // A flat nib held at a fixed 45° angle.
                    let (nx, ny) = if kind == BrushKind::CalligraphyBrush {
                        (std::f32::consts::FRAC_1_SQRT_2, -std::f32::consts::FRAC_1_SQRT_2)
                    } else {
                        (-std::f32::consts::FRAC_1_SQRT_2, -std::f32::consts::FRAC_1_SQRT_2)
                    };
                    let (hx, hy) = (nx * radius, ny * radius);
                    // Give the nib a little thickness so strokes along its edge stay visible.
                    let (tx, ty) = (-ny * 0.6, nx * 0.6);
                    let quad = |dx: f32, dy: f32| {
                        [
                            Point::from_xy(a.x - hx + dx, a.y - hy + dy),
                            Point::from_xy(a.x + hx + dx, a.y + hy + dy),
                            Point::from_xy(b.x + hx + dx, b.y + hy + dy),
                            Point::from_xy(b.x - hx + dx, b.y - hy + dy),
                        ]
                    };
                    fill_polygon(draft, &quad(tx, ty), color);
                    fill_polygon(draft, &quad(-tx, -ty), color);
                    let hull = convex_hull([quad(tx, ty), quad(-tx, -ty)].concat());
                    fill_polygon(draft, &hull, color);
                    segment_bounds(draft, a, b, radius + 2.0)
                }
                BrushKind::Airbrush => {
                    let radius = self.airbrush_radius();
                    let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
                    let steps = (len / (radius / 2.0)).ceil().max(1.0) as usize;
                    let mut dirty = None;
                    for i in 0..=steps {
                        let t = i as f32 / steps as f32;
                        let p = Point::from_xy(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
                        let dots = (radius * 1.2).ceil() as usize;
                        dirty = raster::union_opt(dirty, self.spray(draft, p, radius, dots));
                    }
                    dirty
                }
                BrushKind::Marker => {
                    // A square chisel tip swept along the segment.
                    let h = radius.max(0.5);
                    let corners = |p: Point| {
                        [
                            Point::from_xy(p.x - h, p.y - h),
                            Point::from_xy(p.x + h, p.y - h),
                            Point::from_xy(p.x + h, p.y + h),
                            Point::from_xy(p.x - h, p.y + h),
                        ]
                    };
                    let hull = convex_hull([corners(a), corners(b)].concat());
                    fill_polygon(draft, &hull, color);
                    segment_bounds(draft, a, b, h + 2.0)
                }
                BrushKind::Crayon => capsule(draft, a, b, radius, |x, y, _, _| {
                    let wax = value_noise(x as f32, y as f32, 2.5, 11) * 0.6 + grain(x, y, 7) * 0.4;
                    (((wax - 0.22) * 2.8).clamp(0.0, 1.0), color)
                }),
                BrushKind::NaturalPencil => {
                    let r = (radius * 0.75).max(0.75);
                    capsule(draft, a, b, r, |x, y, d, _| {
                        let g = grain(x, y, 3) * 0.7 + value_noise(x as f32, y as f32, 1.7, 5) * 0.3;
                        let core = 1.0 - d * 0.35;
                        ((g * 0.5 + 0.45) * core, color)
                    })
                }
                BrushKind::Watercolor => capsule(draft, a, b, radius, |x, y, d, _| {
                    // A translucent wash with soft edges and pigment granulation.
                    let soft = ((1.0 - d) / 0.35).clamp(0.0, 1.0);
                    let soft = soft * soft * (3.0 - 2.0 * soft);
                    let n = value_noise(x as f32, y as f32, 7.0, 21) * 0.2 + grain(x, y, 23) * 0.08;
                    ((0.42 + n) * soft, color)
                }),
                BrushKind::OilBrush => {
                    // Paint runs out over the length of the stroke.
                    let load = (1.0 - self.travelled / (size * 80.0 + 2000.0)).clamp(0.6, 1.0);
                    let bristles = &self.bristles;
                    let count = bristles.len().max(1) as f32;
                    capsule(draft, a, b, radius, |x, y, _, across| {
                        let nearest = bristles
                            .iter()
                            .min_by(|p, q| (p.offset - across).abs().total_cmp(&(q.offset - across).abs()));
                        let Some(br) = nearest else {
                            return (1.0, color);
                        };
                        let gap = ((br.offset - across).abs() * count).min(1.0);
                        let streak = value_noise(x as f32, y as f32, 3.0, 31) * 0.1;
                        let alpha = br.alpha * load * (1.0 - gap * 0.2) + 0.08 - streak * 0.5;
                        (alpha.clamp(0.0, 1.0), shade(color, br.shade))
                    })
                }
            },
        }
    }

    /// Renders a complete polyline in one go; shapes use this for textured outlines.
    pub fn draw_polyline(&mut self, draft: &mut Pixmap, points: &[Point]) -> Option<IntRect> {
        let mut dirty = None;
        for p in points {
            dirty = raster::union_opt(dirty, self.add_point(draft, p.x, p.y));
        }
        dirty
    }
}

/// Commits a finished draft onto a layer. Only `region` is touched.
pub fn apply_draft(layer: &mut Pixmap, draft: &Pixmap, region: IntRect, paint: &PixmapPaint) {
    let patch = raster::copy_region(draft, region);
    layer.draw_pixmap(region.x(), region.y(), patch.as_ref(), paint, Transform::identity(), None);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::raster::{content_bounds, get_pixel, new_pixmap};

    fn draw(kind: StrokeKind, size: f32, pts: &[(f32, f32)]) -> (Pixmap, Option<IntRect>) {
        let mut draft = new_pixmap(100, 100);
        let mut s = Stroke::new(kind, Rgba::BLACK, size, 42);
        let mut dirty = None;
        for &(x, y) in pts {
            dirty = raster::union_opt(dirty, s.add_point(&mut draft, x, y));
        }
        (draft, dirty)
    }

    #[test]
    fn pencil_is_aliased_and_exact() {
        let (draft, _) = draw(StrokeKind::Pencil, 1.0, &[(10.5, 10.5), (20.5, 10.5)]);
        assert_eq!(content_bounds(&draft), IntRect::from_xywh(10, 10, 11, 1));
        for x in 10..=20 {
            assert_eq!(get_pixel(&draft, x, 10), Some(Rgba::BLACK));
        }
    }

    #[test]
    fn single_click_leaves_a_dot() {
        for kind in [StrokeKind::Pencil, StrokeKind::Eraser]
            .into_iter()
            .chain(BrushKind::ALL.into_iter().map(StrokeKind::Brush))
        {
            let (draft, dirty) = draw(kind, 8.0, &[(50.0, 50.0)]);
            assert!(content_bounds(&draft).is_some(), "{kind:?} left no mark");
            let dirty = dirty.expect("dirty rect");
            let content = content_bounds(&draft).unwrap();
            assert_eq!(raster::union(dirty, content), dirty, "{kind:?} drew outside its dirty rect");
        }
    }

    #[test]
    fn every_brush_covers_the_path() {
        for kind in BrushKind::ALL {
            let (draft, dirty) = draw(StrokeKind::Brush(kind), 10.0, &[(20.0, 50.0), (50.0, 50.0), (80.0, 50.0)]);
            let content = content_bounds(&draft).unwrap_or_else(|| panic!("{kind:?} drew nothing"));
            assert!(content.width() >= 55, "{kind:?} too short: {content:?}");
            assert_eq!(raster::union(dirty.unwrap(), content), dirty.unwrap(), "{kind:?}");
        }
    }

    #[test]
    fn airbrush_keeps_spraying() {
        let mut draft = new_pixmap(100, 100);
        let mut s = Stroke::new(StrokeKind::Brush(BrushKind::Airbrush), Rgba::BLACK, 10.0, 1);
        s.add_point(&mut draft, 50.0, 50.0);
        let count = |p: &Pixmap| p.pixels().iter().filter(|c| c.alpha() > 0).count();
        let before = count(&draft);
        s.tick(&mut draft, 0.5);
        assert!(count(&draft) > before);
    }

    #[test]
    fn eraser_composites_destination_out() {
        let mut layer = raster::filled(20, 20, Rgba::WHITE);
        let mut draft = new_pixmap(20, 20);
        let mut s = Stroke::new(StrokeKind::Eraser, Rgba::WHITE, 4.0, 0);
        let r = s.add_point(&mut draft, 10.0, 10.0).unwrap();
        apply_draft(&mut layer, &draft, r, &s.composite_paint());
        assert_eq!(get_pixel(&layer, 10, 10).unwrap().a, 0);
        assert_eq!(get_pixel(&layer, 0, 0), Some(Rgba::WHITE));
    }

    #[test]
    fn marker_is_translucent() {
        let mut layer = raster::filled(20, 20, Rgba::WHITE);
        let mut draft = new_pixmap(20, 20);
        let mut s = Stroke::new(StrokeKind::Brush(BrushKind::Marker), Rgba::BLACK, 6.0, 0);
        let r = s.add_point(&mut draft, 10.0, 10.0).unwrap();
        apply_draft(&mut layer, &draft, r, &s.composite_paint());
        let c = get_pixel(&layer, 10, 10).unwrap();
        assert!(c.r > 60 && c.r < 200, "marker should be translucent, got {c:?}");
    }
}

#[cfg(test)]
mod hull_tests {
    use super::*;

    #[test]
    fn hull_of_duplicated_square() {
        let sq = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let pts: Vec<Point> = sq.iter().chain(sq.iter()).map(|&(x, y)| Point::from_xy(x, y)).collect();
        assert_eq!(convex_hull(pts).len(), 4);
    }

    #[test]
    fn hull_drops_interior_points() {
        let pts = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0), (2.0, 2.0), (1.0, 3.0)]
            .iter()
            .map(|&(x, y)| Point::from_xy(x, y))
            .collect();
        assert_eq!(convex_hull(pts).len(), 4);
    }
}
