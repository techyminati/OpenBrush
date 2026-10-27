// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Writes the app icon as PNG files for packaging.
//!
//! cargo run --example render_icon -- assets/icons-app

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "assets/app-icon".into()));
    std::fs::create_dir_all(&dir).expect("create output directory");
    for size in [16, 32, 48, 64, 128, 256, 512, 1024] {
        let path = dir.join(format!("icon-{size}.png"));
        openbrush::ui::app_icon::render(size).save_png(&path).expect("write png");
        println!("wrote {}", path.display());
    }
}
