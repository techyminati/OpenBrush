// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Reports how long system fonts take to load and which default text font is chosen.
//!
//! cargo run --release --example font_probe

fn main() {
    let start = std::time::Instant::now();
    let mut engine = openbrush::engine::text::TextEngine::new();
    println!("loaded {} families in {:?}", engine.families().len(), start.elapsed());
    let family = engine.default_family();
    println!("default text font: {family}");
    let style = openbrush::engine::text::TextStyle {
        family,
        size_pt: 11.0,
        bold: false,
        italic: false,
        underline: false,
        strikethrough: false,
        align: Default::default(),
        color: openbrush::engine::color::Rgba::BLACK,
        background: None,
    };
    let start = std::time::Instant::now();
    let mut target = openbrush::engine::raster::new_pixmap(400, 100);
    engine.render(&mut target, "The quick brown fox", &style, 0, 0, 400, 100);
    println!("rendered a line in {:?}", start.elapsed());
}
