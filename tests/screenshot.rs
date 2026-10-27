// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Renders the app to PNG files for visual review: `OPENBRUSH_SHOTS=dir cargo test --test screenshot`.

use egui_kittest::Harness;
use openbrush::app::{AppOptions, PaintApp};
use openbrush::ui::theme::ThemePref;

fn shoot(name: &str, dark: bool) {
    let Some(dir) = std::env::var_os("OPENBRUSH_SHOTS") else {
        return;
    };
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1360.0, 820.0))
        .with_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light })
        .wgpu()
        .build_eframe(|cc| {
            let mut app = PaintApp::with_options(cc, AppOptions { load_system_fonts: false });
            app.theme = if dark { ThemePref::Dark } else { ThemePref::Light };
            app
        });
    harness.run_steps(4);
    let image = harness.render().expect("render");
    let path = std::path::Path::new(&dir).join(format!("{name}.png"));
    image.save(&path).expect("save");
    println!("wrote {}", path.display());
}

#[test]
fn light_window() {
    shoot("light", false);
}

#[test]
fn dark_window() {
    shoot("dark", true);
}
