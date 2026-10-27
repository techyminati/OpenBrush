// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Renders every shape and outline/fill style to a PNG, for eyeballing shape changes.
//!
//! cargo run --example shape_gallery -- out.png

use openbrush::engine::brush::apply_draft;
use openbrush::engine::color::Rgba;
use openbrush::engine::raster;
use openbrush::engine::shapes::{Geometry, PaintStyle, ShapeKind, ShapeStyle, render};
use tiny_skia::Point;

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "shape_gallery.png".into());
    let cell = 90.0;
    let cols = 8;
    let rows = 3 + 2;
    let mut canvas = raster::filled((cell * cols as f32) as u32, (cell * rows as f32) as u32, Rgba::WHITE);
    let solid = ShapeStyle {
        outline: PaintStyle::Solid,
        fill: PaintStyle::Solid,
        outline_color: Rgba::rgb(63, 72, 204),
        fill_color: Rgba::rgb(153, 217, 234),
        width: 3.0,
    };
    let origin = |i: usize| (((i % cols) as f32) * cell + 10.0, ((i / cols) as f32) * cell + 10.0);
    for (i, kind) in ShapeKind::ALL.into_iter().enumerate() {
        let (x, y) = origin(i);
        let (a, b) = (Point::from_xy(x, y), Point::from_xy(x + cell - 20.0, y + cell - 20.0));
        let geometry = match kind {
            ShapeKind::Line => Geometry::Line { a, b },
            ShapeKind::Curve => Geometry::Curve {
                a: Point::from_xy(x, y + 60.0),
                b: Point::from_xy(x + 70.0, y + 10.0),
                c1: Point::from_xy(x + 10.0, y - 20.0),
                c2: Point::from_xy(x + 60.0, y + 90.0),
            },
            ShapeKind::Polygon => Geometry::Polygon {
                points: vec![a, Point::from_xy(x + 70.0, y + 20.0), b, Point::from_xy(x + 20.0, y + 50.0)],
                closed: true,
            },
            kind => Geometry::Boxed { kind, a, b },
        };
        render(&mut canvas, &geometry, &solid, i as u64);
    }
    for (i, style) in PaintStyle::ALL.into_iter().enumerate().skip(1) {
        for (row, fill) in [(3, PaintStyle::None), (4, style)] {
            let (x, y) = origin(row * cols + i);
            let g = Geometry::Boxed {
                kind: ShapeKind::FivePointStar,
                a: Point::from_xy(x, y),
                b: Point::from_xy(x + cell - 20.0, y + cell - 20.0),
            };
            let s = ShapeStyle { outline: style, fill, width: 6.0, outline_color: Rgba::rgb(237, 28, 36), fill_color: Rgba::rgb(255, 201, 14) };
            // Shapes are drawn into a transparent draft, then composited, as in the app.
            let mut draft = raster::new_pixmap(canvas.width(), canvas.height());
            if let Some(dirty) = render(&mut draft, &g, &s, i as u64) {
                apply_draft(&mut canvas, &draft, dirty, &tiny_skia::PixmapPaint::default());
            }
        }
    }
    canvas.save_png(&out).expect("write png");
    println!("wrote {out}");
}
