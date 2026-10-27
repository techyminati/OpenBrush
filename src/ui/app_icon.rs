// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! The OpenBrush app icon — an artist's palette with a brush — drawn in code so
//! every size is crisp.

use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Paint, Path, PathBuilder, Pixmap, Point, RadialGradient, Rect, Shader,
    SpreadMode, Stroke, Transform,
};

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba8(r, g, b, a)
}

fn solid(c: Color) -> Paint<'static> {
    Paint {
        shader: Shader::SolidColor(c),
        anti_alias: true,
        ..Paint::default()
    }
}

fn linear(from: (f32, f32), to: (f32, f32), stops: &[(f32, Color)]) -> Paint<'static> {
    let stops = stops.iter().map(|&(p, c)| GradientStop::new(p, c)).collect();
    let shader = LinearGradient::new(Point::from_xy(from.0, from.1), Point::from_xy(to.0, to.1), stops, SpreadMode::Pad, Transform::identity())
        .unwrap_or(Shader::SolidColor(Color::WHITE));
    Paint { shader, anti_alias: true, ..Paint::default() }
}

fn oval(cx: f32, cy: f32, rx: f32, ry: f32) -> Option<Path> {
    PathBuilder::from_oval(Rect::from_xywh(cx - rx, cy - ry, rx * 2.0, ry * 2.0)?)
}

/// The palette outline (with its thumb hole), in a 256-unit design space.
fn palette() -> Option<Path> {
    let mut pb = PathBuilder::new();
    pb.move_to(30.0, 132.0);
    pb.cubic_to(26.0, 78.0, 84.0, 40.0, 146.0, 42.0);
    pb.cubic_to(206.0, 44.0, 238.0, 82.0, 230.0, 122.0);
    pb.cubic_to(224.0, 150.0, 196.0, 150.0, 180.0, 160.0);
    pb.cubic_to(162.0, 172.0, 178.0, 198.0, 160.0, 214.0);
    pb.cubic_to(132.0, 236.0, 70.0, 226.0, 46.0, 192.0);
    pb.cubic_to(36.0, 176.0, 31.0, 156.0, 30.0, 132.0);
    pb.close();
    // Thumb hole.
    pb.push_oval(Rect::from_xywh(122.0, 168.0, 30.0, 24.0)?);
    pb.finish()
}

/// Renders the icon at `size` × `size` pixels.
pub fn render(size: u32) -> Pixmap {
    let mut pm = Pixmap::new(size, size).expect("icon size is non-zero");
    let scale = size as f32 / 256.0;
    // The whole palette is tilted slightly, like a palette resting in a hand.
    let base = Transform::from_scale(scale, scale).pre_concat(Transform::from_rotate_at(-12.0, 128.0, 132.0));

    let Some(shape) = palette() else {
        return pm;
    };

    // Soft drop shadow: a few offset, translucent copies.
    for (i, alpha) in [(6.0, 14u8), (4.0, 20), (2.5, 26)] {
        let t = base.pre_translate(i * 0.4, i);
        pm.fill_path(&shape, &solid(rgba(0, 0, 0, alpha)), FillRule::EvenOdd, t, None);
    }

    // Body: warm white with a gentle top-to-bottom gradient, and a crisp rim.
    let body = linear((60.0, 40.0), (180.0, 230.0), &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(226, 230, 238, 255))]);
    pm.fill_path(&shape, &body, FillRule::EvenOdd, base, None);
    let rim = Stroke { width: 3.0, ..Stroke::default() };
    pm.stroke_path(&shape, &solid(rgba(160, 168, 184, 255)), &rim, base, None);

    // Paint blobs, each with a darker base and a glossy highlight.
    let blobs: [(f32, f32, f32, (u8, u8, u8)); 5] = [
        (74.0, 112.0, 19.0, (232, 17, 35)),
        (112.0, 76.0, 18.0, (255, 185, 0)),
        (160.0, 68.0, 18.0, (22, 198, 12)),
        (198.0, 128.0, 16.0, (0, 120, 212)),
        (80.0, 168.0, 18.0, (136, 23, 152)),
    ];
    for (x, y, r, (cr, cg, cb)) in blobs {
        let Some(blob) = oval(x, y, r, r * 0.88) else { continue };
        let shader = RadialGradient::new(
            Point::from_xy(x - r * 0.35, y - r * 0.35),
            0.0,
            Point::from_xy(x, y),
            r * 1.2,
            vec![
                GradientStop::new(0.0, rgba(cr.saturating_add(40), cg.saturating_add(40), cb.saturating_add(40), 255)),
                GradientStop::new(1.0, rgba((cr as f32 * 0.78) as u8, (cg as f32 * 0.78) as u8, (cb as f32 * 0.78) as u8, 255)),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap_or(Shader::SolidColor(rgba(cr, cg, cb, 255)));
        pm.fill_path(&blob, &Paint { shader, anti_alias: true, ..Paint::default() }, FillRule::Winding, base, None);
        if let Some(shine) = oval(x - r * 0.35, y - r * 0.4, r * 0.34, r * 0.2) {
            pm.fill_path(&shine, &solid(rgba(255, 255, 255, 170)), FillRule::Winding, base, None);
        }
    }

    // The brush, laid diagonally across the palette (drawn without the tilt).
    let flat = Transform::from_scale(scale, scale);
    let mut pb = PathBuilder::new();
    pb.move_to(236.0, 18.0);
    pb.line_to(150.0, 128.0);
    if let Some(handle) = pb.finish() {
        let shadow = Stroke { width: 18.0, line_cap: tiny_skia::LineCap::Round, ..Stroke::default() };
        pm.stroke_path(&handle, &solid(rgba(0, 0, 0, 40)), &shadow, flat.pre_translate(3.0, 4.0), None);
        let wood = linear((226.0, 20.0), (246.0, 34.0), &[(0.0, rgba(255, 140, 60, 255)), (1.0, rgba(196, 72, 16, 255))]);
        let stroke = Stroke { width: 15.0, line_cap: tiny_skia::LineCap::Round, ..Stroke::default() };
        pm.stroke_path(&handle, &wood, &stroke, flat, None);
    }
    // Metal ferrule.
    let mut pb = PathBuilder::new();
    pb.move_to(160.0, 116.0);
    pb.line_to(138.0, 144.0);
    if let Some(ferrule) = pb.finish() {
        let metal = linear((140.0, 120.0), (158.0, 134.0), &[(0.0, rgba(236, 240, 246, 255)), (1.0, rgba(140, 148, 162, 255))]);
        let stroke = Stroke { width: 19.0, line_cap: tiny_skia::LineCap::Butt, ..Stroke::default() };
        pm.stroke_path(&ferrule, &metal, &stroke, flat, None);
    }
    // Bristles dipped in blue paint, ending in a soft point.
    let mut pb = PathBuilder::new();
    pb.move_to(130.0, 136.0);
    pb.cubic_to(118.0, 150.0, 104.0, 170.0, 86.0, 196.0);
    pb.cubic_to(112.0, 186.0, 140.0, 168.0, 148.0, 150.0);
    pb.close();
    if let Some(tip) = pb.finish() {
        let paint = linear((140.0, 140.0), (92.0, 192.0), &[(0.0, rgba(64, 160, 255, 255)), (1.0, rgba(0, 84, 166, 255))]);
        pm.fill_path(&tip, &paint, FillRule::Winding, flat, None);
    }
    pm
}

/// The icon in the form the window system wants.
pub fn icon_data() -> egui::IconData {
    let pm = render(256);
    egui::IconData {
        rgba: crate::engine::raster::to_rgba8(&pm),
        width: pm.width(),
        height: pm.height(),
    }
}
