// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

fn main() -> eframe::Result {
    let options = eframe::NativeOptions::default();
    eframe::run_ui_native("OpenBrush", options, |ui, _frame| {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.label("OpenBrush");
        });
    })
}
