// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Renders the app in various states to PNG files for visual review:
//! `OPENBRUSH_SHOTS=dir cargo test --test screenshot`. Does nothing otherwise.

use egui::{Event, Modifiers, PointerButton};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use openbrush::app::{AppOptions, Command, PaintApp};
use openbrush::engine::color::Rgba;
use openbrush::ui::theme::ThemePref;

fn harness(dark: bool) -> Option<Harness<'static, PaintApp>> {
    std::env::var_os("OPENBRUSH_SHOTS")?;
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360.0, 820.0))
        .wgpu()
        .build_eframe(move |cc| {
            let mut app = PaintApp::with_options(cc, AppOptions { load_system_fonts: false });
            app.theme = if dark { ThemePref::Dark } else { ThemePref::Light };
            app
        });
    h.run_steps(4);
    Some(h)
}

fn save(h: &mut Harness<PaintApp>, name: &str) {
    let dir = std::env::var_os("OPENBRUSH_SHOTS").expect("checked in harness()");
    h.run_steps(2);
    let path = std::path::Path::new(&dir).join(format!("{name}.png"));
    h.render().expect("render").save(&path).expect("save");
    println!("wrote {}", path.display());
}

fn drag(h: &mut Harness<PaintApp>, points: &[(f32, f32)]) {
    let pos = |h: &Harness<PaintApp>, (x, y): (f32, f32)| h.state().doc_to_screen(x, y);
    let first = pos(h, points[0]);
    h.event(Event::PointerMoved(first));
    h.event(Event::PointerButton { pos: first, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    for p in &points[1..] {
        let q = pos(h, *p);
        h.event(Event::PointerMoved(q));
        h.step();
    }
    let last = pos(h, *points.last().unwrap());
    h.event(Event::PointerButton { pos: last, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
}

fn click(h: &mut Harness<PaintApp>, label: &str) {
    h.get_by_label(label).click();
    h.run_steps(3);
}

fn run(h: &mut Harness<PaintApp>, c: Command) {
    let ctx = h.ctx.clone();
    h.state_mut().execute(&ctx, c);
    h.run_steps(2);
}

/// Draws a small scene with several tools.
fn scene(h: &mut Harness<PaintApp>) {
    h.state_mut().color1 = Rgba::rgb(63, 72, 204);
    click(h, "Brushes");
    click(h, "Oil brush");
    h.state_mut().sizes.brush = 14;
    let wave: Vec<(f32, f32)> = (0..40).map(|i| (80.0 + i as f32 * 12.0, 120.0 + (i as f32 / 4.0).sin() * 30.0)).collect();
    drag(h, &wave);
    h.state_mut().color1 = Rgba::rgb(237, 28, 36);
    h.state_mut().color2 = Rgba::rgb(255, 201, 14);
    click(h, "Five-point star");
    click(h, "Fill");
    click(h, "Solid color");
    drag(h, &[(700.0, 80.0), (900.0, 280.0)]);
    run(h, Command::Commit);
    h.state_mut().color1 = Rgba::rgb(34, 177, 76);
    click(h, "Heart");
    drag(h, &[(200.0, 300.0), (380.0, 480.0)]);
}

#[test]
fn light_window() {
    let Some(mut h) = harness(false) else { return };
    save(&mut h, "light");
}

#[test]
fn dark_window() {
    let Some(mut h) = harness(true) else { return };
    save(&mut h, "dark");
}

#[test]
fn drawing_with_pending_shape() {
    let Some(mut h) = harness(false) else { return };
    scene(&mut h);
    save(&mut h, "scene");
}

#[test]
fn file_menu() {
    let Some(mut h) = harness(false) else { return };
    click(&mut h, "File");
    click(&mut h, "Save as");
    save(&mut h, "file-menu");
}

#[test]
fn brushes_dropdown() {
    let Some(mut h) = harness(true) else { return };
    click(&mut h, "Brushes");
    save(&mut h, "brushes");
}

#[test]
fn outline_dropdown() {
    let Some(mut h) = harness(false) else { return };
    click(&mut h, "Rectangle");
    click(&mut h, "Outline");
    save(&mut h, "outline");
}

#[test]
fn edit_colors_dialog() {
    let Some(mut h) = harness(false) else { return };
    click(&mut h, "Edit colors");
    save(&mut h, "edit-colors");
}

#[test]
fn resize_dialog() {
    let Some(mut h) = harness(true) else { return };
    run(&mut h, Command::ResizeSkew);
    save(&mut h, "resize");
}

#[test]
fn layers_text_and_rulers() {
    let Some(mut h) = harness(false) else { return };
    scene(&mut h);
    run(&mut h, Command::Commit);
    run(&mut h, Command::AddLayer);
    run(&mut h, Command::ToggleRulers);
    run(&mut h, Command::ToggleGrid);
    click(&mut h, "Text");
    h.state_mut().text_style.size_pt = 28.0;
    drag(&mut h, &[(500.0, 400.0), (900.0, 480.0)]);
    h.event(Event::Text("Hello from OpenBrush".into()));
    h.run_steps(2);
    save(&mut h, "layers-text");
}

#[test]
fn selection_overlay() {
    let Some(mut h) = harness(false) else { return };
    scene(&mut h);
    run(&mut h, Command::Commit);
    click(&mut h, "Select");
    h.get_by_role_and_label(egui::accesskit::Role::CheckBox, "Rectangle").click();
    h.run_steps(2);
    drag(&mut h, &[(650.0, 50.0), (950.0, 320.0)]);
    drag(&mut h, &[(800.0, 200.0), (750.0, 300.0), (700.0, 380.0)]);
    save(&mut h, "selection");
}
