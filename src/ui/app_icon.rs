// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! The OpenBrush app icon, drawn in code so every size is crisp.

use tiny_skia::{Color, FillRule, GradientStop, LinearGradient, Paint, PathBuilder, Pixmap, Point, Rect, Shader, SpreadMode, Stroke, Transform};

fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
    let k = 0.552_284_8 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

fn solid(r: u8, g: u8, b: u8) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(r, g, b, 255);
    p
}

/// Renders the icon at `size` × `size` pixels.
pub fn render(size: u32) -> Pixmap {
    let mut pm = Pixmap::new(size, size).expect("icon size is non-zero");
    let s = size as f32 / 256.0;
    let t = Transform::from_scale(s, s);

    // Tile with a blue-to-violet gradient.
    if let Some(tile) = rounded_rect(12.0, 12.0, 232.0, 232.0, 52.0) {
        let shader = LinearGradient::new(
            Point::from_xy(12.0, 12.0),
            Point::from_xy(244.0, 244.0),
            vec![
                GradientStop::new(0.0, Color::from_rgba8(0, 120, 212, 255)),
                GradientStop::new(1.0, Color::from_rgba8(118, 70, 230, 255)),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap_or(Shader::SolidColor(Color::from_rgba8(0, 103, 192, 255)));
        let paint = Paint { shader, ..Paint::default() };
        pm.fill_path(&tile, &paint, FillRule::Winding, t, None);
    }

    // White palette with a thumb hole.
    let mut pb = PathBuilder::new();
    if let Some(oval) = Rect::from_xywh(40.0, 58.0, 176.0, 140.0) {
        pb.push_oval(oval);
    }
    if let Some(hole) = Rect::from_xywh(140.0, 140.0, 36.0, 30.0) {
        pb.push_oval(hole);
    }
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &solid(255, 255, 255), FillRule::EvenOdd, t, None);
    }

    // Paint wells.
    for (x, y, (r, g, b)) in [
        (82.0, 104.0, (237, 28, 36)),
        (122.0, 86.0, (255, 201, 14)),
        (164.0, 98.0, (34, 177, 76)),
        (88.0, 148.0, (0, 162, 232)),
    ] {
        if let Some(dot) = Rect::from_xywh(x - 15.0, y - 15.0, 30.0, 30.0).and_then(PathBuilder::from_oval) {
            pm.fill_path(&dot, &solid(r, g, b), FillRule::Winding, t, None);
        }
    }

    // A brush laid across the palette.
    let mut pb = PathBuilder::new();
    pb.move_to(214.0, 40.0);
    pb.line_to(150.0, 150.0);
    if let Some(handle) = pb.finish() {
        let stroke = Stroke {
            width: 16.0,
            line_cap: tiny_skia::LineCap::Round,
            ..Stroke::default()
        };
        pm.stroke_path(&handle, &solid(60, 44, 32), &stroke, t, None);
    }
    let mut pb = PathBuilder::new();
    pb.move_to(150.0, 150.0);
    pb.cubic_to(136.0, 162.0, 132.0, 186.0, 118.0, 204.0);
    pb.cubic_to(140.0, 202.0, 166.0, 186.0, 168.0, 160.0);
    pb.close();
    if let Some(tip) = pb.finish() {
        pm.fill_path(&tip, &solid(63, 72, 204), FillRule::Winding, t, None);
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
