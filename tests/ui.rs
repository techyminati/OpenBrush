// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! End-to-end tests that drive the real app the way a user would: clicking toolbar
//! buttons by their labels, dragging on the canvas, typing and using shortcuts.

use egui::{Event, Key, Modifiers, PointerButton, Pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use openbrush::app::{AppOptions, Command, PaintApp, Tool};
use openbrush::engine::color::Rgba;

const RED: Rgba = Rgba::rgb(237, 28, 36);

fn app() -> Harness<'static, PaintApp> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360.0, 860.0))
        .build_eframe(|cc| PaintApp::with_options(cc, AppOptions { load_system_fonts: false }));
    h.run_steps(4);
    h
}

fn screen(h: &Harness<PaintApp>, x: f32, y: f32) -> Pos2 {
    h.state().doc_to_screen(x, y)
}

fn press(h: &mut Harness<PaintApp>, x: f32, y: f32, button: PointerButton, pressed: bool) {
    let pos = screen(h, x, y);
    h.event(Event::PointerMoved(pos));
    h.event(Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    });
    h.step();
}

fn drag_with(h: &mut Harness<PaintApp>, points: &[(f32, f32)], button: PointerButton) {
    let (x0, y0) = points[0];
    press(h, x0, y0, button, true);
    for &(x, y) in &points[1..] {
        let pos = screen(h, x, y);
        h.event(Event::PointerMoved(pos));
        h.step();
    }
    let (x1, y1) = *points.last().unwrap();
    press(h, x1, y1, button, false);
    h.run_steps(2);
}

fn drag(h: &mut Harness<PaintApp>, points: &[(f32, f32)]) {
    drag_with(h, points, PointerButton::Primary);
}

fn click(h: &mut Harness<PaintApp>, x: f32, y: f32) {
    drag(h, &[(x, y)]);
}

fn pixel(h: &Harness<PaintApp>, x: i32, y: i32) -> Rgba {
    h.state().pixel(x, y).expect("pixel inside canvas")
}

fn button(h: &mut Harness<PaintApp>, label: &str) {
    h.get_by_label(label).click();
    h.run_steps(3);
}

fn shortcut(h: &mut Harness<PaintApp>, modifiers: Modifiers, key: Key) {
    h.key_press_modifiers(modifiers, key);
    h.run_steps(2);
}

fn run(h: &mut Harness<PaintApp>, command: Command) {
    let ctx = h.ctx.clone();
    h.state_mut().execute(&ctx, command);
    h.run_steps(3);
}

#[test]
fn starts_with_blank_white_canvas() {
    let h = app();
    let s = h.state();
    assert_eq!((s.doc.width(), s.doc.height()), (1152, 648));
    assert_eq!(s.tool, Tool::Pencil);
    assert_eq!(pixel(&h, 10, 10), Rgba::WHITE);
    assert!(!s.is_dirty());
    assert_eq!(s.title(), "Untitled - OpenBrush");
}

#[test]
fn pencil_draws_and_undo_redo_work() {
    let mut h = app();
    drag(&mut h, &[(20.0, 20.0), (60.0, 20.0), (100.0, 20.0)]);
    assert_eq!(pixel(&h, 50, 20), Rgba::BLACK);
    assert_eq!(pixel(&h, 50, 25), Rgba::WHITE);
    assert!(h.state().is_dirty());
    assert!(h.state().title().starts_with('*'));

    shortcut(&mut h, Modifiers::COMMAND, Key::Z);
    assert_eq!(pixel(&h, 50, 20), Rgba::WHITE);
    shortcut(&mut h, Modifiers::COMMAND, Key::Y);
    assert_eq!(pixel(&h, 50, 20), Rgba::BLACK);

    // The Undo button in the menu bar works too.
    button(&mut h, &format!("Undo ({})", openbrush::app::commands_shortcut("Z")));
    assert_eq!(pixel(&h, 50, 20), Rgba::WHITE);
}

#[test]
fn right_button_draws_with_color_2() {
    let mut h = app();
    h.state_mut().color2 = RED;
    drag_with(&mut h, &[(20.0, 40.0), (80.0, 40.0)], PointerButton::Secondary);
    assert_eq!(pixel(&h, 50, 40), RED);
}

#[test]
fn palette_sets_active_color() {
    let mut h = app();
    button(&mut h, "#ED1C24");
    assert_eq!(h.state().color1, RED);
    button(&mut h, "Color 2");
    button(&mut h, "#22B14C");
    assert_eq!(h.state().color2, Rgba::rgb(34, 177, 76));
    assert_eq!(h.state().color1, RED);
}

#[test]
fn every_brush_paints() {
    let mut h = app();
    let brushes = ["Brush", "Calligraphy brush", "Calligraphy pen", "Airbrush", "Oil brush", "Crayon", "Marker", "Natural pencil", "Watercolor brush"];
    for (i, name) in brushes.iter().enumerate() {
        button(&mut h, "Brushes");
        button(&mut h, name);
        assert_eq!(h.state().tool, Tool::Brush, "{name}");
        let y = 40.0 + i as f32 * 40.0;
        let before = h.state().history.state_id();
        drag(&mut h, &[(100.0, y), (160.0, y), (220.0, y), (280.0, y)]);
        assert_ne!(h.state().history.state_id(), before, "{name} recorded no history");
        let marked = (100..280).step_by(4).any(|x| (-6..=6).any(|dy| pixel(&h, x, y as i32 + dy) != Rgba::WHITE));
        assert!(marked, "{name} left no mark");
    }
}

#[test]
fn eraser_erases_to_transparent() {
    let mut h = app();
    button(&mut h, "Eraser");
    drag(&mut h, &[(200.0, 200.0), (260.0, 200.0)]);
    assert_eq!(pixel(&h, 230, 200).a, 0);
    assert_eq!(pixel(&h, 230, 260), Rgba::WHITE);
}

#[test]
fn fill_fills_enclosed_area() {
    let mut h = app();
    // Draw a closed rectangle with the Rectangle shape.
    button(&mut h, "Rectangle");
    drag(&mut h, &[(100.0, 100.0), (200.0, 160.0), (300.0, 200.0)]);
    shortcut(&mut h, Modifiers::NONE, Key::Enter);
    assert_eq!(pixel(&h, 200, 100), Rgba::BLACK, "outline");
    assert_eq!(pixel(&h, 200, 150), Rgba::WHITE, "no fill by default");

    h.state_mut().color1 = RED;
    button(&mut h, "Fill with color");
    click(&mut h, 200.0, 150.0);
    assert_eq!(pixel(&h, 200, 150), RED);
    assert_eq!(pixel(&h, 50, 50), Rgba::WHITE, "fill stayed inside the rectangle");
}

#[test]
fn shapes_can_be_moved_before_committing() {
    let mut h = app();
    button(&mut h, "Oval");
    drag(&mut h, &[(100.0, 100.0), (200.0, 200.0)]);
    // Drag from inside the bounding box to move the pending shape.
    drag(&mut h, &[(150.0, 150.0), (250.0, 150.0), (350.0, 150.0)]);
    shortcut(&mut h, Modifiers::NONE, Key::Enter);
    let ink = |h: &Harness<PaintApp>, x0: i32, x1: i32| (x0..x1).any(|x| pixel(h, x, 150) != Rgba::WHITE);
    assert!(!ink(&h, 95, 190), "the oval should have moved away");
    assert!(ink(&h, 295, 305), "left edge of the moved oval");
}

#[test]
fn shape_fill_and_outline_styles() {
    let mut h = app();
    h.state_mut().color2 = RED;
    button(&mut h, "Five-point star");
    button(&mut h, "Fill");
    button(&mut h, "Solid color");
    drag(&mut h, &[(300.0, 100.0), (500.0, 300.0)]);
    shortcut(&mut h, Modifiers::NONE, Key::Escape);
    assert_eq!(pixel(&h, 400, 210), RED);
}

#[test]
fn line_and_polygon_and_curve() {
    let mut h = app();
    button(&mut h, "Line");
    drag(&mut h, &[(50.0, 300.0), (250.0, 300.0)]);
    shortcut(&mut h, Modifiers::NONE, Key::Enter);
    assert_eq!(pixel(&h, 150, 300), Rgba::BLACK);

    button(&mut h, "Polygon");
    drag(&mut h, &[(400.0, 400.0), (500.0, 400.0)]);
    click(&mut h, 500.0, 500.0);
    click(&mut h, 400.0, 500.0);
    click(&mut h, 400.0, 400.0); // back at the start closes it
    assert!(h.state().history.state_id() > 0);
    assert_eq!(pixel(&h, 450, 400), Rgba::BLACK);
    assert_eq!(pixel(&h, 400, 450), Rgba::BLACK, "closing edge");

    button(&mut h, "Curve");
    drag(&mut h, &[(600.0, 300.0), (800.0, 300.0)]);
    drag(&mut h, &[(700.0, 200.0)]);
    drag(&mut h, &[(700.0, 200.0)]);
    // Both bends pull towards (700, 200), so the curve's middle passes through y = 225.
    assert!((215..236).any(|y| pixel(&h, 700, y) != Rgba::WHITE), "curve bent upwards");
    assert_eq!(pixel(&h, 700, 300), Rgba::WHITE, "curve no longer straight");
}

#[test]
fn selection_moves_pixels() {
    let mut h = app();
    h.state_mut().color1 = RED;
    button(&mut h, "Rectangle");
    button(&mut h, "Fill");
    button(&mut h, "Solid color");
    h.state_mut().color2 = RED;
    drag(&mut h, &[(100.0, 100.0), (150.0, 150.0)]);
    shortcut(&mut h, Modifiers::NONE, Key::Enter);
    assert_eq!(pixel(&h, 125, 125), RED);

    button(&mut h, "Select");
    h.get_by_role_and_label(egui::accesskit::Role::CheckBox, "Rectangle").click();
    h.run_steps(3);
    assert_eq!(h.state().tool, Tool::Select);
    drag(&mut h, &[(90.0, 90.0), (160.0, 160.0)]);
    assert!(h.state().has_selection());
    drag(&mut h, &[(125.0, 125.0), (225.0, 125.0), (325.0, 125.0)]);
    shortcut(&mut h, Modifiers::NONE, Key::Enter);
    assert!(!h.state().has_selection());
    assert_eq!(pixel(&h, 125, 125).a, 0, "source left transparent");
    assert_eq!(pixel(&h, 325, 125), RED);

    // One undo restores the original position.
    shortcut(&mut h, Modifiers::COMMAND, Key::Z);
    assert_eq!(pixel(&h, 125, 125), RED);
    assert_eq!(pixel(&h, 325, 125), Rgba::WHITE);
}

#[test]
fn select_all_copy_paste_and_delete() {
    let mut h = app();
    drag(&mut h, &[(10.0, 10.0), (40.0, 10.0)]);
    run(&mut h, Command::SelectAll);
    assert!(h.state().has_selection());
    let pixels = h.state().selected_pixels().expect("selection pixels");
    assert_eq!((pixels.width(), pixels.height()), (1152, 648));

    // Paste an image directly (the system clipboard isn't available in CI).
    let mut red = openbrush::engine::raster::filled(20, 20, RED);
    openbrush::engine::raster::set_pixel(&mut red, 0, 0, Rgba::BLACK);
    run(&mut h, Command::Commit);
    h.state_mut().paste_image(red);
    h.run_steps(2);
    assert!(h.state().has_selection());
    assert_eq!(h.state().tool, Tool::Select);
    run(&mut h, Command::Commit);
    assert_eq!(pixel(&h, 5, 5), RED);

    run(&mut h, Command::SelectAll);
    shortcut(&mut h, Modifiers::NONE, Key::Delete);
    assert_eq!(pixel(&h, 5, 5).a, 0);
}

#[test]
fn free_form_selection_and_invert() {
    let mut h = app();
    button(&mut h, "Select");
    button(&mut h, "Free-form");
    drag(&mut h, &[(100.0, 100.0), (300.0, 100.0), (300.0, 300.0), (100.0, 300.0), (100.0, 101.0)]);
    assert!(h.state().has_selection());
    run(&mut h, Command::InvertSelection);
    shortcut(&mut h, Modifiers::NONE, Key::Delete);
    assert_eq!(pixel(&h, 200, 200), Rgba::WHITE, "inside kept");
    assert_eq!(pixel(&h, 50, 50).a, 0, "outside deleted");
}

#[test]
fn text_tool_types_text() {
    let mut h = app();
    button(&mut h, "Text");
    click(&mut h, 100.0, 100.0);
    assert!(h.get_by_label("Bold").rect().width() > 0.0, "text toolbar shown");
    h.state_mut().text_style.size_pt = 36.0;
    h.event(Event::Text("Hello".into()));
    h.run_steps(2);
    h.key_press(Key::Backspace);
    h.event(Event::Text("o!".into()));
    h.run_steps(2);
    shortcut(&mut h, Modifiers::NONE, Key::Escape);
    let dark = (100..260).flat_map(|x| (100..160).map(move |y| (x, y))).filter(|&(x, y)| pixel(&h, x, y).r < 100).count();
    assert!(dark > 50, "text should have been drawn ({dark} dark pixels)");
    assert!(h.state().history.can_undo());
}

#[test]
fn color_picker_picks_and_returns_to_previous_tool() {
    let mut h = app();
    h.state_mut().color1 = RED;
    drag(&mut h, &[(30.0, 30.0), (60.0, 30.0)]);
    h.state_mut().color1 = Rgba::BLACK;
    button(&mut h, "Color picker");
    click(&mut h, 45.0, 30.0);
    assert_eq!(h.state().color1, RED);
    assert_eq!(h.state().tool, Tool::Pencil);
}

#[test]
fn magnifier_and_zoom_commands() {
    let mut h = app();
    let start = h.state().view.zoom;
    button(&mut h, "Magnifier");
    click(&mut h, 300.0, 300.0);
    assert!(h.state().view.zoom > start);
    run(&mut h, Command::ZoomActual);
    assert_eq!(h.state().view.zoom, 1.0);
    run(&mut h, Command::ZoomIn);
    assert_eq!(h.state().view.zoom, 2.0);
    shortcut(&mut h, Modifiers::COMMAND, Key::PageDown);
    assert_eq!(h.state().view.zoom, 1.0);
    run(&mut h, Command::ZoomFit);
    assert!(h.state().view.zoom <= 1.0);
}

#[test]
fn image_rotate_flip_resize_crop() {
    let mut h = app();
    button(&mut h, "Rotate");
    button(&mut h, "Rotate right 90°");
    assert_eq!((h.state().doc.width(), h.state().doc.height()), (648, 1152));
    shortcut(&mut h, Modifiers::COMMAND, Key::Z);
    assert_eq!((h.state().doc.width(), h.state().doc.height()), (1152, 648));

    h.state_mut().resize_and_skew(576, 324, 0.0, 0.0);
    assert_eq!((h.state().doc.width(), h.state().doc.height()), (576, 324));

    button(&mut h, "Select");
    h.get_by_role_and_label(egui::accesskit::Role::CheckBox, "Rectangle").click();
    h.run_steps(3);
    drag(&mut h, &[(10.0, 10.0), (110.0, 60.0)]);
    run(&mut h, Command::Crop);
    assert_eq!((h.state().doc.width(), h.state().doc.height()), (100, 50));
}

#[test]
fn resize_skew_dialog_applies_percentage() {
    let mut h = app();
    shortcut(&mut h, Modifiers::COMMAND, Key::W);
    assert!(h.state().has_dialog());
    h.get_by_label("OK").click();
    h.run_steps(3);
    assert!(!h.state().has_dialog());
    assert_eq!((h.state().doc.width(), h.state().doc.height()), (1152, 648), "100% keeps the size");
}

#[test]
fn canvas_can_be_resized_by_dragging_handle() {
    let mut h = app();
    let (w, hgt) = (h.state().doc.width() as f32, h.state().doc.height() as f32);
    let zoom = h.state().view.zoom;
    // The bottom-right handle sits just outside the canvas corner.
    let offset = 7.0 / zoom;
    drag(&mut h, &[(w + offset - 0.5, hgt + offset - 0.5), (w + 50.0, hgt + 20.0), (w - 100.0, hgt - 100.0)]);
    let s = h.state();
    assert!(s.doc.width() < 1152 && s.doc.height() < 648, "got {}×{}", s.doc.width(), s.doc.height());
}

#[test]
fn layers_add_hide_and_merge() {
    let mut h = app();
    button(&mut h, "Layers");
    assert!(h.state().view.show_layers);
    button(&mut h, "Add layer");
    assert_eq!(h.state().doc.layers.len(), 2);
    h.state_mut().color1 = RED;
    drag(&mut h, &[(40.0, 40.0), (80.0, 40.0)]);
    assert_eq!(pixel(&h, 60, 40), RED);
    assert_eq!(openbrush::engine::raster::get_pixel(&h.state().doc.layers[0].pixels, 60, 40), Some(Rgba::WHITE), "drew on the new layer");

    // Cards are listed top layer first, so the first "Hide layer" is the new layer's.
    h.get_all_by_label("Hide layer").next().expect("eye button").click();
    h.run_steps(3);
    assert!(!h.state().doc.layers[1].visible);
    assert_eq!(pixel(&h, 60, 40), Rgba::WHITE);
    button(&mut h, "Show layer");
    assert_eq!(pixel(&h, 60, 40), RED);

    run(&mut h, Command::MergeDown);
    assert_eq!(h.state().doc.layers.len(), 1);
    assert_eq!(pixel(&h, 60, 40), RED);
}

#[test]
fn edit_colors_dialog_sets_color_and_custom_slot() {
    let mut h = app();
    button(&mut h, "Edit colors");
    assert!(h.state().has_dialog());
    let field = h.get_by_role(egui::accesskit::Role::TextInput);
    field.focus();
    h.run_steps(1);
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.event(Event::Text("#3366CC".into()));
    h.run_steps(2);
    h.get_by_label("OK").click();
    h.run_steps(3);
    assert!(!h.state().has_dialog());
    assert_eq!(h.state().color1, Rgba::rgb(0x33, 0x66, 0xCC));
    assert_eq!(h.state().custom_colors.first(), Some(&Rgba::rgb(0x33, 0x66, 0xCC)));
}

#[test]
fn save_open_round_trip_and_unsaved_prompt() {
    let dir = std::env::temp_dir().join(format!("openbrush-ui-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("drawing.png");

    let mut h = app();
    h.state_mut().color1 = RED;
    drag(&mut h, &[(10.0, 10.0), (50.0, 10.0)]);
    assert!(h.state_mut().save_to(&path, openbrush::engine::io::FileFormat::Png));
    h.run_steps(2);
    assert!(!h.state().is_dirty());
    assert_eq!(h.state().title(), "drawing.png - OpenBrush");
    assert!(h.state().file.size_on_disk.unwrap() > 0);

    // A change, then New: should ask to save first.
    drag(&mut h, &[(10.0, 30.0), (50.0, 30.0)]);
    run(&mut h, Command::New);
    assert!(h.state().has_dialog());
    h.get_by_label("Don't save").click();
    h.run_steps(3);
    assert!(!h.state().has_dialog());
    assert_eq!(pixel(&h, 30, 10), Rgba::WHITE, "new blank picture");

    assert!(h.state_mut().open_path(&path));
    h.run_steps(2);
    assert_eq!(pixel(&h, 30, 10), RED);
    assert_eq!(pixel(&h, 30, 30), Rgba::WHITE, "unsaved stroke was discarded");
    assert_eq!(h.state().recent.first().map(|p| p.file_name().unwrap().to_owned()), Some("drawing.png".into()));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn opening_a_bad_file_shows_an_error() {
    let dir = std::env::temp_dir().join(format!("openbrush-bad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bad.png");
    std::fs::write(&path, b"not an image").unwrap();
    let mut h = app();
    assert!(!h.state_mut().open_path(&path));
    h.run_steps(2);
    assert!(h.state().has_dialog());
    h.get_by_label("OK").click();
    h.run_steps(2);
    assert!(!h.state().has_dialog());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn menus_open_and_run_commands() {
    let mut h = app();
    button(&mut h, "View");
    button(&mut h, "Show");
    button(&mut h, "Rulers");
    assert!(h.state().view.show_rulers);
    button(&mut h, "File");
    button(&mut h, "Image properties");
    assert!(h.state().has_dialog());
    h.get_by_label("Cancel").click();
    h.run_steps(3);
    assert!(!h.state().has_dialog());
    button(&mut h, "Edit");
    button(&mut h, "Select all");
    assert!(h.state().has_selection());
}

#[test]
fn theme_can_be_switched() {
    let mut h = app();
    button(&mut h, "Settings");
    button(&mut h, "Dark");
    assert!(h.ctx.global_style().visuals.dark_mode);
    button(&mut h, "Settings");
    button(&mut h, "Light");
    assert!(!h.ctx.global_style().visuals.dark_mode);
}

#[test]
fn size_dropdown_changes_pencil_width() {
    let mut h = app();
    button(&mut h, "Size");
    button(&mut h, "4 px");
    assert_eq!(h.state().sizes.pencil, 4);
    drag(&mut h, &[(300.0, 300.0), (340.0, 300.0)]);
    assert_eq!(pixel(&h, 320, 301), Rgba::BLACK);
    shortcut(&mut h, Modifiers::COMMAND, Key::Equals);
    assert_eq!(h.state().sizes.pencil, 5);
}
