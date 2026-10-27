// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Renders a sample stroke with every brush to a PNG, for eyeballing brush changes.
//!
//! cargo run --example brush_gallery -- out.png

use openbrush::engine::brush::{BrushKind, Stroke, StrokeKind, apply_draft};
use openbrush::engine::color::Rgba;
use openbrush::engine::raster;

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "brush_gallery.png".into());
    let kinds: Vec<StrokeKind> = [StrokeKind::Pencil]
        .into_iter()
        .chain(BrushKind::ALL.into_iter().map(StrokeKind::Brush))
        .collect();
    let row_h = 70.0;
    let mut canvas = raster::filled(720, (row_h as usize * kinds.len()) as u32, Rgba::WHITE);
    let colors = [Rgba::rgb(63, 72, 204), Rgba::rgb(237, 28, 36)];
    for (row, kind) in kinds.iter().enumerate() {
        for (i, size) in [4.0f32, 16.0].into_iter().enumerate() {
            let mut draft = raster::new_pixmap(canvas.width(), canvas.height());
            let mut stroke = Stroke::new(*kind, colors[i], size, row as u64);
            let base_y = row as f32 * row_h + 22.0 + i as f32 * 26.0;
            let mut dirty = None;
            for step in 0..=120 {
                let x = 20.0 + step as f32 * 5.6;
                let y = base_y + (step as f32 / 9.0).sin() * 10.0;
                dirty = raster::union_opt(dirty, stroke.add_point(&mut draft, x, y));
                if step % 30 == 0 {
                    dirty = raster::union_opt(dirty, stroke.tick(&mut draft, 0.05));
                }
            }
            apply_draft(&mut canvas, &draft, dirty.unwrap(), &stroke.composite_paint());
        }
    }
    canvas.save_png(&out).expect("write png");
    println!("wrote {out}");
}
