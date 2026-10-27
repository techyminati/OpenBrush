fn main() -> eframe::Result {
    let options = eframe::NativeOptions::default();
    eframe::run_simple_native("OpenBrush", options, |ctx, _frame| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("OpenBrush");
        });
    })
}
