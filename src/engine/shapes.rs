// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Shape geometry and rendering for the Shapes gallery.

use tiny_skia::{FillRule, IntRect, LineCap, LineJoin, Mask, Paint, Path, PathBuilder, PathSegment, Pixmap, Point, Rect, Transform};

use super::brush::{self, BrushKind, Stroke, StrokeKind};
use super::color::Rgba;
use super::raster;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum ShapeKind {
    Line,
    Curve,
    Oval,
    Rectangle,
    RoundedRectangle,
    Polygon,
    Triangle,
    RightTriangle,
    Diamond,
    Pentagon,
    Hexagon,
    RightArrow,
    LeftArrow,
    UpArrow,
    DownArrow,
    FourPointStar,
    FivePointStar,
    SixPointStar,
    RoundedRectangularCallout,
    OvalCallout,
    CloudCallout,
    Heart,
    Lightning,
}

impl ShapeKind {
    pub const ALL: [ShapeKind; 23] = [
        ShapeKind::Line,
        ShapeKind::Curve,
        ShapeKind::Oval,
        ShapeKind::Rectangle,
        ShapeKind::RoundedRectangle,
        ShapeKind::Polygon,
        ShapeKind::Triangle,
        ShapeKind::RightTriangle,
        ShapeKind::Diamond,
        ShapeKind::Pentagon,
        ShapeKind::Hexagon,
        ShapeKind::RightArrow,
        ShapeKind::LeftArrow,
        ShapeKind::UpArrow,
        ShapeKind::DownArrow,
        ShapeKind::FourPointStar,
        ShapeKind::FivePointStar,
        ShapeKind::SixPointStar,
        ShapeKind::RoundedRectangularCallout,
        ShapeKind::OvalCallout,
        ShapeKind::CloudCallout,
        ShapeKind::Heart,
        ShapeKind::Lightning,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ShapeKind::Line => "Line",
            ShapeKind::Curve => "Curve",
            ShapeKind::Oval => "Oval",
            ShapeKind::Rectangle => "Rectangle",
            ShapeKind::RoundedRectangle => "Rounded rectangle",
            ShapeKind::Polygon => "Polygon",
            ShapeKind::Triangle => "Triangle",
            ShapeKind::RightTriangle => "Right triangle",
            ShapeKind::Diamond => "Diamond",
            ShapeKind::Pentagon => "Pentagon",
            ShapeKind::Hexagon => "Hexagon",
            ShapeKind::RightArrow => "Right arrow",
            ShapeKind::LeftArrow => "Left arrow",
            ShapeKind::UpArrow => "Up arrow",
            ShapeKind::DownArrow => "Down arrow",
            ShapeKind::FourPointStar => "Four-point star",
            ShapeKind::FivePointStar => "Five-point star",
            ShapeKind::SixPointStar => "Six-point star",
            ShapeKind::RoundedRectangularCallout => "Rounded rectangular callout",
            ShapeKind::OvalCallout => "Oval callout",
            ShapeKind::CloudCallout => "Cloud callout",
            ShapeKind::Heart => "Heart",
            ShapeKind::Lightning => "Lightning",
        }
    }

    /// Shapes defined by a bounding box (everything except line, curve and polygon).
    pub fn is_boxed(self) -> bool {
        !matches!(self, ShapeKind::Line | ShapeKind::Curve | ShapeKind::Polygon)
    }

    /// Whether the shape can be filled.
    pub fn is_closed(self) -> bool {
        !matches!(self, ShapeKind::Line | ShapeKind::Curve)
    }
}

/// Texture used for an outline or a fill. `Solid` uses a flat colour; the others mimic brushes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum PaintStyle {
    None,
    Solid,
    Crayon,
    Marker,
    Oil,
    NaturalPencil,
    Watercolor,
}

impl PaintStyle {
    pub const ALL: [PaintStyle; 7] = [
        PaintStyle::None,
        PaintStyle::Solid,
        PaintStyle::Crayon,
        PaintStyle::Marker,
        PaintStyle::Oil,
        PaintStyle::NaturalPencil,
        PaintStyle::Watercolor,
    ];

    pub fn outline_label(self) -> &'static str {
        match self {
            PaintStyle::None => "No outline",
            PaintStyle::Solid => "Solid color",
            PaintStyle::Crayon => "Crayon",
            PaintStyle::Marker => "Marker",
            PaintStyle::Oil => "Oil",
            PaintStyle::NaturalPencil => "Natural pencil",
            PaintStyle::Watercolor => "Watercolor",
        }
    }

    pub fn fill_label(self) -> &'static str {
        match self {
            PaintStyle::None => "No fill",
            other => other.outline_label(),
        }
    }

    fn brush(self) -> Option<BrushKind> {
        match self {
            PaintStyle::Crayon => Some(BrushKind::Crayon),
            PaintStyle::Marker => Some(BrushKind::Marker),
            PaintStyle::Oil => Some(BrushKind::OilBrush),
            PaintStyle::NaturalPencil => Some(BrushKind::NaturalPencil),
            PaintStyle::Watercolor => Some(BrushKind::Watercolor),
            PaintStyle::None | PaintStyle::Solid => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ShapeStyle {
    pub outline: PaintStyle,
    pub fill: PaintStyle,
    pub outline_color: Rgba,
    pub fill_color: Rgba,
    pub width: f32,
}

/// The geometry of a shape being drawn.
#[derive(Clone, PartialEq, Debug)]
pub enum Geometry {
    /// A gallery shape fitted to the box spanned by two corners.
    Boxed { kind: ShapeKind, a: Point, b: Point },
    Line { a: Point, b: Point },
    /// A cubic Bézier: Paint's curve tool bends a line twice.
    Curve { a: Point, b: Point, c1: Point, c2: Point },
    Polygon { points: Vec<Point>, closed: bool },
}

impl Geometry {
    pub fn path(&self) -> Option<Path> {
        match self {
            Geometry::Boxed { kind, a, b } => {
                let r = Rect::from_ltrb(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y))?;
                boxed_path(*kind, r)
            }
            Geometry::Line { a, b } => {
                let mut pb = PathBuilder::new();
                pb.move_to(a.x, a.y);
                pb.line_to(b.x, b.y);
                // Zero-length lines still need a visible dot.
                if a == b {
                    pb.line_to(b.x + 0.01, b.y);
                }
                pb.finish()
            }
            Geometry::Curve { a, b, c1, c2 } => {
                let mut pb = PathBuilder::new();
                pb.move_to(a.x, a.y);
                pb.cubic_to(c1.x, c1.y, c2.x, c2.y, b.x + 0.01, b.y);
                pb.finish()
            }
            Geometry::Polygon { points, closed } => {
                let (first, rest) = points.split_first()?;
                let mut pb = PathBuilder::new();
                pb.move_to(first.x, first.y);
                for p in rest {
                    pb.line_to(p.x, p.y);
                }
                if rest.is_empty() {
                    pb.line_to(first.x + 0.01, first.y);
                }
                if *closed {
                    pb.close();
                }
                pb.finish()
            }
        }
    }

    pub fn is_closed(&self) -> bool {
        match self {
            Geometry::Boxed { .. } => true,
            Geometry::Polygon { closed, .. } => *closed,
            Geometry::Line { .. } | Geometry::Curve { .. } => false,
        }
    }

    /// Moves every point by `(dx, dy)`.
    pub fn translate(&mut self, dx: f32, dy: f32) {
        self.map_points(|p| Point::from_xy(p.x + dx, p.y + dy));
    }

    pub fn map_points(&mut self, f: impl Fn(Point) -> Point) {
        match self {
            Geometry::Boxed { a, b, .. } | Geometry::Line { a, b } => {
                *a = f(*a);
                *b = f(*b);
            }
            Geometry::Curve { a, b, c1, c2 } => {
                for p in [a, b, c1, c2] {
                    *p = f(*p);
                }
            }
            Geometry::Polygon { points, .. } => {
                for p in points {
                    *p = f(*p);
                }
            }
        }
    }

    /// The box the user manipulates with handles.
    pub fn bounds(&self) -> Option<Rect> {
        match self {
            Geometry::Boxed { a, b, .. } | Geometry::Line { a, b } => {
                Rect::from_ltrb(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y))
            }
            _ => self.path().map(|p| p.bounds()),
        }
    }
}

/// Builds a path from points given in a reference box, then fits it to `r`.
fn fitted(r: Rect, build: impl FnOnce(&mut PathBuilder)) -> Option<Path> {
    let mut pb = PathBuilder::new();
    build(&mut pb);
    let path = pb.finish()?;
    let b = path.bounds();
    let sx = r.width() / b.width().max(1e-3);
    let sy = r.height() / b.height().max(1e-3);
    path.transform(Transform::from_row(sx, 0.0, 0.0, sy, r.left() - b.left() * sx, r.top() - b.top() * sy))
}

fn polygon(pb: &mut PathBuilder, pts: &[(f32, f32)]) {
    pb.move_to(pts[0].0, pts[0].1);
    for &(x, y) in &pts[1..] {
        pb.line_to(x, y);
    }
    pb.close();
}

fn star(pb: &mut PathBuilder, points: usize, inner: f32) {
    let n = points * 2;
    let pts: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            let angle = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / n as f32;
            let r = if i % 2 == 0 { 1.0 } else { inner };
            (r * angle.cos(), r * angle.sin())
        })
        .collect();
    polygon(pb, &pts);
}

fn regular_polygon(pb: &mut PathBuilder, sides: usize) {
    let pts: Vec<(f32, f32)> = (0..sides)
        .map(|i| {
            let angle = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / sides as f32;
            (angle.cos(), angle.sin())
        })
        .collect();
    polygon(pb, &pts);
}

/// Points on an ellipse from `start` to `end` (radians, screen orientation), inclusive.
fn ellipse_arc(cx: f32, cy: f32, rx: f32, ry: f32, start: f32, end: f32, steps: usize) -> Vec<(f32, f32)> {
    (0..=steps)
        .map(|i| {
            let t = start + (end - start) * i as f32 / steps as f32;
            (cx + rx * t.cos(), cy + ry * t.sin())
        })
        .collect()
}

fn boxed_path(kind: ShapeKind, r: Rect) -> Option<Path> {
    match kind {
        ShapeKind::Oval => PathBuilder::from_oval(r),
        ShapeKind::Rectangle => Some(PathBuilder::from_rect(r)),
        ShapeKind::RoundedRectangle => {
            let radius = (r.width().min(r.height()) / 6.0).max(0.0);
            let mut pb = PathBuilder::new();
            push_round_rect(&mut pb, r, radius);
            pb.finish()
        }
        ShapeKind::Triangle => fitted(r, |pb| polygon(pb, &[(0.5, 0.0), (1.0, 1.0), (0.0, 1.0)])),
        ShapeKind::RightTriangle => fitted(r, |pb| polygon(pb, &[(0.0, 0.0), (1.0, 1.0), (0.0, 1.0)])),
        ShapeKind::Diamond => fitted(r, |pb| polygon(pb, &[(0.5, 0.0), (1.0, 0.5), (0.5, 1.0), (0.0, 0.5)])),
        ShapeKind::Pentagon => fitted(r, |pb| regular_polygon(pb, 5)),
        ShapeKind::Hexagon => fitted(r, |pb| {
            polygon(pb, &[(0.25, 0.0), (0.75, 0.0), (1.0, 0.5), (0.75, 1.0), (0.25, 1.0), (0.0, 0.5)])
        }),
        ShapeKind::RightArrow | ShapeKind::LeftArrow | ShapeKind::UpArrow | ShapeKind::DownArrow => {
            let base = [(0.0, 0.25), (0.55, 0.25), (0.55, 0.0), (1.0, 0.5), (0.55, 1.0), (0.55, 0.75), (0.0, 0.75)];
            let pts: Vec<(f32, f32)> = base
                .iter()
                .map(|&(x, y)| match kind {
                    ShapeKind::RightArrow => (x, y),
                    ShapeKind::LeftArrow => (1.0 - x, y),
                    ShapeKind::UpArrow => (y, 1.0 - x),
                    _ => (y, x),
                })
                .collect();
            fitted(r, |pb| polygon(pb, &pts))
        }
        ShapeKind::FourPointStar => fitted(r, |pb| star(pb, 4, 0.3)),
        ShapeKind::FivePointStar => fitted(r, |pb| star(pb, 5, 0.382)),
        ShapeKind::SixPointStar => fitted(r, |pb| star(pb, 6, 0.577)),
        ShapeKind::RoundedRectangularCallout => fitted(r, |pb| {
            // Body occupies the top 75%; the tail hangs from the bottom-left.
            let (w, h, rad) = (1.0, 0.75, 0.12);
            let mut pts = Vec::new();
            pts.extend(ellipse_arc(rad, rad, rad, rad, std::f32::consts::PI, 1.5 * std::f32::consts::PI, 8));
            pts.extend(ellipse_arc(w - rad, rad, rad, rad, -std::f32::consts::FRAC_PI_2, 0.0, 8));
            pts.extend(ellipse_arc(w - rad, h - rad, rad, rad, 0.0, std::f32::consts::FRAC_PI_2, 8));
            pts.extend([(0.4, h), (0.2, 1.0), (0.22, h)]);
            pts.extend(ellipse_arc(rad, h - rad, rad, rad, std::f32::consts::FRAC_PI_2, std::f32::consts::PI, 8));
            polygon(pb, &pts);
        }),
        ShapeKind::OvalCallout => fitted(r, |pb| {
            // Ellipse body with a tail that leaves between 105° and 130°.
            let (a0, a1) = (105f32.to_radians(), 130f32.to_radians());
            let mut pts = vec![(0.12, 1.0)];
            pts.extend(ellipse_arc(0.5, 0.4, 0.5, 0.4, a0, a1 - std::f32::consts::TAU, 72));
            polygon(pb, &pts);
        }),
        ShapeKind::CloudCallout => fitted(r, |pb| {
            // Puffy outline: arcs bulging outward between points on an ellipse.
            let (cx, cy, rx, ry) = (0.5, 0.4, 0.44, 0.32);
            let bumps = 10;
            let point = |i: usize, scale: f32| {
                let t = i as f32 / bumps as f32 * std::f32::consts::TAU;
                (cx + rx * scale * t.cos(), cy + ry * scale * t.sin())
            };
            let start = point(0, 1.0);
            pb.move_to(start.0, start.1);
            for i in 0..bumps {
                let (x1, y1) = point(i + 1, 1.0);
                let t0 = i as f32 / bumps as f32 * std::f32::consts::TAU;
                let t1 = (i + 1) as f32 / bumps as f32 * std::f32::consts::TAU;
                let ctrl = |t: f32| (cx + rx * 1.32 * t.cos(), cy + ry * 1.32 * t.sin());
                let c1 = ctrl(t0 + (t1 - t0) * 0.15);
                let c2 = ctrl(t0 + (t1 - t0) * 0.85);
                pb.cubic_to(c1.0, c1.1, c2.0, c2.1, x1, y1);
            }
            pb.close();
            // Thought bubbles trailing to the bottom-left.
            for (bx, by, br) in [(0.2, 0.84, 0.06), (0.1, 0.95, 0.035)] {
                if let Some(oval) = Rect::from_ltrb(bx - br, by - br, bx + br, by + br) {
                    pb.push_oval(oval);
                }
            }
        }),
        ShapeKind::Heart => fitted(r, |pb| {
            pb.move_to(50.0, 28.0);
            pb.cubic_to(50.0, 10.0, 25.0, 0.0, 10.0, 12.0);
            pb.cubic_to(-5.0, 25.0, 2.0, 55.0, 50.0, 95.0);
            pb.cubic_to(98.0, 55.0, 105.0, 25.0, 90.0, 12.0);
            pb.cubic_to(75.0, 0.0, 50.0, 10.0, 50.0, 28.0);
            pb.close();
        }),
        ShapeKind::Lightning => fitted(r, |pb| {
            polygon(
                pb,
                &[
                    (8458.0, 0.0),
                    (0.0, 3923.0),
                    (7564.0, 8416.0),
                    (4993.0, 9720.0),
                    (12197.0, 13904.0),
                    (9987.0, 14934.0),
                    (21600.0, 21600.0),
                    (14768.0, 12911.0),
                    (16558.0, 12016.0),
                    (11030.0, 6840.0),
                    (12831.0, 6120.0),
                ],
            )
        }),
        ShapeKind::Line | ShapeKind::Curve | ShapeKind::Polygon => None,
    }
}

fn push_round_rect(pb: &mut PathBuilder, r: Rect, radius: f32) {
    let k = 0.552_284_8 * radius;
    let (l, t, rt, b) = (r.left(), r.top(), r.right(), r.bottom());
    pb.move_to(l + radius, t);
    pb.line_to(rt - radius, t);
    pb.cubic_to(rt - radius + k, t, rt, t + radius - k, rt, t + radius);
    pb.line_to(rt, b - radius);
    pb.cubic_to(rt, b - radius + k, rt - radius + k, b, rt - radius, b);
    pb.line_to(l + radius, b);
    pb.cubic_to(l + radius - k, b, l, b - radius + k, l, b - radius);
    pb.line_to(l, t + radius);
    pb.cubic_to(l, t + radius - k, l + radius - k, t, l + radius, t);
    pb.close();
}

/// Splits a path into polylines (one per sub-path), sampling curves finely enough to look smooth.
pub fn flatten(path: &Path) -> Vec<(Vec<Point>, bool)> {
    let mut out: Vec<(Vec<Point>, bool)> = Vec::new();
    let mut current: Vec<Point> = Vec::new();
    let mut last = Point::zero();
    let steps_for = |len: f32| (len / 3.0).ceil().clamp(4.0, 200.0) as usize;
    for seg in path.segments() {
        match seg {
            PathSegment::MoveTo(p) => {
                if current.len() > 1 {
                    out.push((std::mem::take(&mut current), false));
                }
                current.clear();
                current.push(p);
                last = p;
            }
            PathSegment::LineTo(p) => {
                current.push(p);
                last = p;
            }
            PathSegment::QuadTo(c, p) => {
                let n = steps_for(last.distance(c) + c.distance(p));
                for i in 1..=n {
                    let t = i as f32 / n as f32;
                    let u = 1.0 - t;
                    current.push(Point::from_xy(
                        u * u * last.x + 2.0 * u * t * c.x + t * t * p.x,
                        u * u * last.y + 2.0 * u * t * c.y + t * t * p.y,
                    ));
                }
                last = p;
            }
            PathSegment::CubicTo(c1, c2, p) => {
                let n = steps_for(last.distance(c1) + c1.distance(c2) + c2.distance(p));
                for i in 1..=n {
                    let t = i as f32 / n as f32;
                    let u = 1.0 - t;
                    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    current.push(Point::from_xy(
                        a * last.x + b * c1.x + c * c2.x + d * p.x,
                        a * last.y + b * c1.y + c * c2.y + d * p.y,
                    ));
                }
                last = p;
            }
            PathSegment::Close => {
                if let Some(&first) = current.first() {
                    current.push(first);
                    last = first;
                }
                if current.len() > 1 {
                    out.push((std::mem::take(&mut current), true));
                }
                current.clear();
            }
        }
    }
    if current.len() > 1 {
        out.push((current, false));
    }
    out
}

/// Draws a shape into `draft`. Returns the area that may have changed.
pub fn render(draft: &mut Pixmap, geometry: &Geometry, style: &ShapeStyle, seed: u64) -> Option<IntRect> {
    let path = geometry.path()?;
    let pad = style.width + 4.0;
    let b = path.bounds();
    let area = raster::rect_around(b.left(), b.top(), b.right(), b.bottom(), pad)
        .and_then(|r| raster::clamp_to(r, draft.width(), draft.height()))?;

    if geometry.is_closed() {
        render_fill(draft, &path, style.fill, style.fill_color, area);
    }
    render_outline(draft, &path, geometry, style, seed);
    Some(area)
}

fn render_fill(draft: &mut Pixmap, path: &Path, fill: PaintStyle, color: Rgba, area: IntRect) {
    match fill {
        PaintStyle::None => {}
        PaintStyle::Solid => {
            let mut paint = Paint::default();
            paint.set_color(color.to_skia());
            draft.fill_path(path, &paint, FillRule::Winding, Transform::identity(), None);
        }
        textured => {
            let Some(mut mask) = Mask::new(draft.width(), draft.height()) else {
                return;
            };
            mask.fill_path(path, FillRule::Winding, true, Transform::identity());
            let w = draft.width() as usize;
            let coverage = mask.data();
            for y in area.top()..area.bottom() {
                for x in area.left()..area.right() {
                    let c = coverage[y as usize * w + x as usize] as f32 / 255.0;
                    if c == 0.0 {
                        continue;
                    }
                    let (xf, yf) = (x as f32, y as f32);
                    let alpha = match textured {
                        PaintStyle::Crayon => {
                            let wax = brush::value_noise(xf, yf, 2.5, 11) * 0.6 + brush::grain(x, y, 7) * 0.4;
                            ((wax - 0.22) * 2.8).clamp(0.0, 1.0)
                        }
                        PaintStyle::Marker => 0.55,
                        PaintStyle::Oil => {
                            let streak = brush::value_noise(xf * 0.25, yf * 2.0, 1.0, 31);
                            0.8 + streak * 0.2
                        }
                        PaintStyle::NaturalPencil => {
                            // Diagonal hatching with graphite grain.
                            let hatch = ((xf + yf) * 0.7).sin() * 0.5 + 0.5;
                            (hatch * 0.5 + brush::grain(x, y, 3) * 0.35).clamp(0.0, 1.0)
                        }
                        PaintStyle::Watercolor => {
                            0.35 + brush::value_noise(xf, yf, 7.0, 21) * 0.2 + brush::grain(x, y, 23) * 0.06
                        }
                        PaintStyle::None | PaintStyle::Solid => 1.0,
                    };
                    raster::max_blend_pixel(draft, x, y, color, (alpha * c * 255.0).round() as u8);
                }
            }
        }
    }
}

fn render_outline(draft: &mut Pixmap, path: &Path, geometry: &Geometry, style: &ShapeStyle, seed: u64) {
    match style.outline {
        PaintStyle::None => {}
        PaintStyle::Solid => {
            let mut paint = Paint::default();
            paint.set_color(style.outline_color.to_skia());
            let open = !geometry.is_closed();
            let stroke = tiny_skia::Stroke {
                width: style.width.max(1.0),
                line_cap: if open { LineCap::Round } else { LineCap::Butt },
                line_join: if matches!(geometry, Geometry::Boxed { kind: ShapeKind::Rectangle, .. }) {
                    LineJoin::Miter
                } else {
                    LineJoin::Round
                },
                ..Default::default()
            };
            draft.stroke_path(path, &paint, &stroke, Transform::identity(), None);
        }
        textured => {
            let Some(kind) = textured.brush() else {
                return;
            };
            // Brush textures are drawn into their own layer, then composited with the
            // brush's own opacity, exactly as a freehand stroke would be.
            let mut layer = raster::new_pixmap(draft.width(), draft.height());
            let mut dirty = None;
            let mut paint = None;
            for (i, (points, _)) in flatten(path).into_iter().enumerate() {
                let mut stroke = Stroke::new(StrokeKind::Brush(kind), style.outline_color, style.width.max(1.0), seed + i as u64);
                dirty = raster::union_opt(dirty, stroke.draw_polyline(&mut layer, &points));
                paint = Some(stroke.composite_paint());
            }
            if let (Some(dirty), Some(paint)) = (dirty, paint) {
                brush::apply_draft(draft, &layer, dirty, &paint);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::raster::{content_bounds, get_pixel, new_pixmap};

    fn style(outline: PaintStyle, fill: PaintStyle) -> ShapeStyle {
        ShapeStyle {
            outline,
            fill,
            outline_color: Rgba::BLACK,
            fill_color: Rgba::rgb(255, 0, 0),
            width: 3.0,
        }
    }

    #[test]
    fn every_boxed_shape_fits_its_box() {
        for kind in ShapeKind::ALL.into_iter().filter(|k| k.is_boxed()) {
            let g = Geometry::Boxed {
                kind,
                a: Point::from_xy(80.0, 70.0),
                b: Point::from_xy(20.0, 10.0),
            };
            let b = g.path().unwrap_or_else(|| panic!("{kind:?} has no path")).bounds();
            assert!((b.left() - 20.0).abs() < 0.5 && (b.right() - 80.0).abs() < 0.5, "{kind:?} x: {b:?}");
            assert!((b.top() - 10.0).abs() < 0.5 && (b.bottom() - 70.0).abs() < 0.5, "{kind:?} y: {b:?}");
        }
    }

    #[test]
    fn filled_rectangle() {
        let mut draft = new_pixmap(100, 100);
        let g = Geometry::Boxed {
            kind: ShapeKind::Rectangle,
            a: Point::from_xy(10.0, 10.0),
            b: Point::from_xy(50.0, 40.0),
        };
        render(&mut draft, &g, &style(PaintStyle::Solid, PaintStyle::Solid), 0).unwrap();
        assert_eq!(get_pixel(&draft, 30, 25), Some(Rgba::rgb(255, 0, 0)));
        assert_eq!(get_pixel(&draft, 10, 25), Some(Rgba::BLACK));
        assert_eq!(get_pixel(&draft, 70, 70).unwrap().a, 0);
    }

    #[test]
    fn outline_only_leaves_inside_empty() {
        let mut draft = new_pixmap(100, 100);
        let g = Geometry::Boxed {
            kind: ShapeKind::Oval,
            a: Point::from_xy(10.0, 10.0),
            b: Point::from_xy(90.0, 90.0),
        };
        render(&mut draft, &g, &style(PaintStyle::Solid, PaintStyle::None), 0).unwrap();
        assert_eq!(get_pixel(&draft, 50, 50).unwrap().a, 0);
        assert_eq!(get_pixel(&draft, 50, 10).unwrap().a, 255);
    }

    #[test]
    fn all_styles_render_inside_dirty_rect() {
        for outline in PaintStyle::ALL {
            for fill in PaintStyle::ALL {
                let mut draft = new_pixmap(120, 120);
                let g = Geometry::Boxed {
                    kind: ShapeKind::Heart,
                    a: Point::from_xy(20.0, 20.0),
                    b: Point::from_xy(100.0, 100.0),
                };
                let dirty = render(&mut draft, &g, &style(outline, fill), 3).unwrap();
                match content_bounds(&draft) {
                    Some(content) => assert_eq!(raster::union(dirty, content), dirty, "{outline:?}/{fill:?}"),
                    None => assert!(outline == PaintStyle::None && fill == PaintStyle::None),
                }
            }
        }
    }

    #[test]
    fn lines_and_curves_are_open() {
        let mut draft = new_pixmap(100, 100);
        let g = Geometry::Curve {
            a: Point::from_xy(10.0, 50.0),
            b: Point::from_xy(90.0, 50.0),
            c1: Point::from_xy(30.0, 0.0),
            c2: Point::from_xy(70.0, 100.0),
        };
        render(&mut draft, &g, &style(PaintStyle::Solid, PaintStyle::Solid), 0).unwrap();
        // Fill must not apply to an open curve.
        assert!(raster::to_rgba8(&draft).chunks(4).all(|c| c[0] == 0 || c[3] == 0));
        assert!(content_bounds(&draft).is_some());
    }

    #[test]
    fn flatten_closes_subpaths() {
        let path = PathBuilder::from_rect(Rect::from_xywh(0.0, 0.0, 10.0, 10.0).unwrap());
        let polys = flatten(&path);
        assert_eq!(polys.len(), 1);
        assert!(polys[0].1);
        assert_eq!(polys[0].0.first(), polys[0].0.last());
    }
}
