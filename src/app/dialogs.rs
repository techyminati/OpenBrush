// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Modal dialogs in the Windows 11 ContentDialog style.

use egui::{Align, Color32, CornerRadius, Frame, Id, Layout, Margin, Mesh, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use tiny_skia::Pixmap;

use super::files::{PendingAction, list_printers};
use super::state::ColorSlot;
use super::{APP_NAME, PaintApp};
use crate::engine::color::Rgba;
use crate::engine::document;
use crate::ui::theme::{self, Palette};
use crate::ui::widgets::{self, to_color32};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Units {
    Pixels,
    Inches,
    Centimeters,
}

impl Units {
    fn per_pixel(self) -> f64 {
        match self {
            Units::Pixels => 1.0,
            Units::Inches => 1.0 / 96.0,
            Units::Centimeters => 2.54 / 96.0,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Units::Pixels => "Pixels",
            Units::Inches => "Inches",
            Units::Centimeters => "Centimeters",
        }
    }
}

pub(crate) struct ResizeSkew {
    by_percent: bool,
    horizontal: f64,
    vertical: f64,
    keep_aspect: bool,
    skew_h: f64,
    skew_v: f64,
    width: u32,
    height: u32,
}

pub(crate) struct Properties {
    units: Units,
    /// Always kept in pixels; converted for display.
    width: f64,
    height: f64,
}

pub(crate) struct EditColors {
    hue: f32,
    saturation: f32,
    value: f32,
    hex: String,
    slot: ColorSlot,
    original: Rgba,
}

impl EditColors {
    fn color(&self) -> Rgba {
        Rgba::from_hsv(self.hue, self.saturation, self.value)
    }

    fn set_color(&mut self, c: Rgba) {
        let (h, s, v) = c.to_hsv();
        // Keep hue/saturation when they become undefined (greys, black).
        if s > 0.0 && v > 0.0 {
            self.hue = h;
        }
        if v > 0.0 {
            self.saturation = s;
        }
        self.value = v;
        self.hex = c.to_hex();
    }
}

pub(crate) struct Print {
    printers: Vec<String>,
    printer: usize,
    copies: u32,
    fit: bool,
    landscape: bool,
}

pub(crate) enum Dialog {
    ConfirmSave { then: PendingAction },
    Message { title: String, body: String },
    About,
    PasteLarger { image: Pixmap },
    ResizeSkew(ResizeSkew),
    Properties(Properties),
    EditColors(EditColors),
    Print(Print),
}

impl Dialog {
    pub(crate) fn message(title: &str, body: &str) -> Self {
        Dialog::Message {
            title: title.to_owned(),
            body: body.to_owned(),
        }
    }

    pub(crate) fn resize_skew(app: &PaintApp) -> Self {
        let (width, height) = match app.selection.as_ref() {
            Some(s) => {
                let b = s.sel.bounds();
                (b.width().round() as u32, b.height().round() as u32)
            }
            None => (app.doc.width(), app.doc.height()),
        };
        Dialog::ResizeSkew(ResizeSkew {
            by_percent: true,
            horizontal: 100.0,
            vertical: 100.0,
            keep_aspect: true,
            skew_h: 0.0,
            skew_v: 0.0,
            width,
            height,
        })
    }

    pub(crate) fn properties(app: &PaintApp) -> Self {
        Dialog::Properties(Properties {
            units: Units::Pixels,
            width: app.doc.width() as f64,
            height: app.doc.height() as f64,
        })
    }

    pub(crate) fn edit_colors(app: &PaintApp) -> Self {
        let slot = app.active_slot;
        let original = if slot == ColorSlot::Primary { app.color1 } else { app.color2 };
        let mut state = EditColors {
            hue: 0.0,
            saturation: 0.0,
            value: 0.0,
            hex: String::new(),
            slot,
            original,
        };
        state.set_color(original);
        Dialog::EditColors(state)
    }

    pub(crate) fn print() -> Self {
        Dialog::Print(Print {
            printers: list_printers(),
            printer: 0,
            copies: 1,
            fit: true,
            landscape: false,
        })
    }
}

/// What the user chose in a dialog's footer.
enum Choice {
    None,
    Primary,
    Secondary,
    Cancel,
}

/// Draws a ContentDialog: title and body on top, buttons on a tinted footer band.
fn content_dialog(
    ctx: &egui::Context,
    id: &str,
    width: f32,
    title: &str,
    buttons: (&str, Option<&str>, Option<&str>),
    primary_enabled: bool,
    body: impl FnOnce(&mut Ui),
) -> Choice {
    let p = Palette::of(ctx);
    let mut choice = Choice::None;
    let frame = Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(8))
        .shadow(ctx.global_style().visuals.window_shadow);
    let response = egui::Modal::new(Id::unique(id))
        .frame(frame)
        .backdrop_color(Color32::from_black_alpha(if p.dark { 120 } else { 70 }))
        .show(ctx, |ui| {
            ui.set_width(width);
            Frame::new().inner_margin(Margin { left: 24, right: 24, top: 22, bottom: 20 }).show(ui, |ui| {
                ui.set_width(width - 48.0);
                ui.label(egui::RichText::new(title).font(theme::semibold(20.0)).color(p.text));
                ui.add_space(12.0);
                body(ui);
            });
            Frame::new()
                .fill(p.surface_alt)
                .corner_radius(CornerRadius { nw: 0, ne: 0, sw: 8, se: 8 })
                .inner_margin(Margin::same(20))
                .show(ui, |ui| {
                    ui.set_width(width - 40.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let (primary, secondary, cancel) = buttons;
                        if let Some(cancel) = cancel {
                            if widgets::dialog_button(ui, cancel, false, true).clicked() {
                                choice = Choice::Cancel;
                            }
                        }
                        if let Some(secondary) = secondary {
                            if widgets::dialog_button(ui, secondary, false, true).clicked() {
                                choice = Choice::Secondary;
                            }
                        }
                        if widgets::dialog_button(ui, primary, true, primary_enabled).clicked() {
                            choice = Choice::Primary;
                        }
                    });
                });
        });
    if matches!(choice, Choice::None) {
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) || response.should_close() {
            choice = if buttons.2.is_some() || buttons.1.is_some() { Choice::Cancel } else { Choice::Primary };
        } else if primary_enabled && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            choice = Choice::Primary;
        }
    }
    choice
}

fn number_row(ui: &mut Ui, label: &str, value: &mut f64, range: std::ops::RangeInclusive<f64>, decimals: usize, suffix: &str) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized(vec2(110.0, 30.0), egui::Label::new(label));
        changed = ui
            .add_sized(
                vec2(120.0, 30.0),
                egui::DragValue::new(value).range(range).max_decimals(decimals).speed(0.5).suffix(suffix),
            )
            .changed();
    });
    changed
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} bytes")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

pub(crate) fn format_file_size(bytes: u64) -> String {
    format_size(bytes)
}

impl PaintApp {
    pub(crate) fn show_dialogs(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.dialog.take() else {
            return;
        };
        let keep = match &mut dialog {
            Dialog::ConfirmSave { then } => {
                let name = self.file.display_name();
                match content_dialog(ctx, "confirm-save", 420.0, APP_NAME, ("Save", Some("Don't save"), Some("Cancel")), true, |ui| {
                    ui.label(format!("Do you want to save your work to {name}?"));
                }) {
                    Choice::Primary => {
                        let then = then.clone();
                        self.dialog = None;
                        if self.save() {
                            self.perform(then);
                        }
                        return;
                    }
                    Choice::Secondary => {
                        let then = then.clone();
                        self.history.clear();
                        self.file.saved_state = self.history.state_id();
                        self.perform(then);
                        return;
                    }
                    Choice::Cancel => false,
                    Choice::None => true,
                }
            }
            Dialog::Message { title, body } => {
                !matches!(content_dialog(ctx, "message", 420.0, title, ("OK", None, None), true, |ui| { ui.label(body.as_str()); }), Choice::Primary | Choice::Cancel)
            }
            Dialog::About => {
                let p = Palette::of(ctx);
                let choice = content_dialog(ctx, "about", 440.0, &format!("About {APP_NAME}"), ("OK", None, None), true, |ui| {
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(56.0), Sense::hover());
                        widgets::paint_hue_wheel(ui.painter(), rect.center(), 26.0);
                        ui.painter().circle_filled(rect.center(), 10.0, p.surface);
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(APP_NAME).font(theme::semibold(16.0)));
                            ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                        });
                    });
                    ui.add_space(10.0);
                    ui.label("A cross-platform paint program modelled on Windows 11 Paint, written in Rust.");
                    ui.add_space(8.0);
                    ui.label("© 2019 – 2026 The CipherOS Project. All Rights Reserved.");
                    ui.label("Author: Aryan Sinha (techyminati)");
                    ui.hyperlink_to("cipheros.org.in", "https://cipheros.org.in");
                    ui.hyperlink_to("github.com/techyminati/openbrush", "https://github.com/techyminati/openbrush");
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Licensed under the CipherOS License 2.0. Uses Fluent UI System Icons (MIT) and the Selawik font (OFL).").font(theme::caption()).color(p.text_secondary));
                });
                !matches!(choice, Choice::Primary | Choice::Cancel)
            }
            Dialog::PasteLarger { image } => {
                let choice = content_dialog(ctx, "paste-larger", 440.0, APP_NAME, ("Enlarge canvas", Some("Keep canvas size"), Some("Cancel")), true, |ui| {
                    ui.label("The image you're pasting is larger than the canvas. Would you like the canvas to be enlarged?");
                });
                match choice {
                    Choice::Primary => {
                        let image = std::mem::replace(image, crate::engine::raster::new_pixmap(1, 1));
                        self.enlarge_and_paste(image);
                        false
                    }
                    Choice::Secondary => {
                        let image = std::mem::replace(image, crate::engine::raster::new_pixmap(1, 1));
                        self.place_pasted(image);
                        false
                    }
                    Choice::Cancel => false,
                    Choice::None => true,
                }
            }
            Dialog::ResizeSkew(state) => self.resize_skew_dialog(ctx, state),
            Dialog::Properties(state) => self.properties_dialog(ctx, state),
            Dialog::EditColors(state) => self.edit_colors_dialog(ctx, state),
            Dialog::Print(state) => self.print_dialog(ctx, state),
        };
        if keep && self.dialog.is_none() {
            self.dialog = Some(dialog);
        }
    }

    fn resize_skew_dialog(&mut self, ctx: &egui::Context, s: &mut ResizeSkew) -> bool {
        let p = Palette::of(ctx);
        let choice = content_dialog(ctx, "resize-skew", 380.0, "Resize and skew", ("OK", None, Some("Cancel")), true, |ui| {
            ui.label(egui::RichText::new("Resize").font(theme::semibold(14.0)));
            ui.horizontal(|ui| {
                ui.label("By:");
                let was = s.by_percent;
                ui.radio_value(&mut s.by_percent, true, "Percentage");
                ui.radio_value(&mut s.by_percent, false, "Pixels");
                if was != s.by_percent {
                    if s.by_percent {
                        s.horizontal = 100.0;
                        s.vertical = 100.0;
                    } else {
                        s.horizontal = s.width as f64;
                        s.vertical = s.height as f64;
                    }
                }
            });
            let (max, suffix) = if s.by_percent { (500.0, " %") } else { (document::MAX_DIMENSION as f64, " px") };
            let ratio = s.height as f64 / s.width as f64;
            if number_row(ui, "Horizontal", &mut s.horizontal, 1.0..=max, 0, suffix) && s.keep_aspect {
                s.vertical = if s.by_percent { s.horizontal } else { (s.horizontal * ratio).round().max(1.0) };
            }
            if number_row(ui, "Vertical", &mut s.vertical, 1.0..=max, 0, suffix) && s.keep_aspect {
                s.horizontal = if s.by_percent { s.vertical } else { (s.vertical / ratio).round().max(1.0) };
            }
            ui.checkbox(&mut s.keep_aspect, "Maintain aspect ratio");
            ui.add_space(10.0);
            ui.label(egui::RichText::new("Skew (degrees)").font(theme::semibold(14.0)));
            number_row(ui, "Horizontal", &mut s.skew_h, -89.0..=89.0, 0, "°");
            number_row(ui, "Vertical", &mut s.skew_v, -89.0..=89.0, 0, "°");
            ui.label(egui::RichText::new(format!("Current size: {} × {} px", s.width, s.height)).font(theme::caption()).color(p.text_secondary));
        });
        match choice {
            Choice::Primary => {
                let (w, h) = if s.by_percent {
                    ((s.width as f64 * s.horizontal / 100.0).round(), (s.height as f64 * s.vertical / 100.0).round())
                } else {
                    (s.horizontal.round(), s.vertical.round())
                };
                let max = document::MAX_DIMENSION as f64;
                self.resize_and_skew(w.clamp(1.0, max) as u32, h.clamp(1.0, max) as u32, s.skew_h as f32, s.skew_v as f32);
                false
            }
            Choice::Cancel | Choice::Secondary => false,
            Choice::None => true,
        }
    }

    fn properties_dialog(&mut self, ctx: &egui::Context, s: &mut Properties) -> bool {
        let p = Palette::of(ctx);
        let saved = self
            .file
            .modified
            .map(|t| {
                let secs = t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                format_timestamp(secs)
            })
            .unwrap_or_else(|| "Not available".to_owned());
        let disk = self.file.size_on_disk.map(format_size).unwrap_or_else(|| "Not available".to_owned());
        let choice = content_dialog(ctx, "properties", 400.0, "Image properties", ("OK", Some("Default"), Some("Cancel")), true, |ui| {
            egui::Grid::new("attrs").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
                ui.colored_label(p.text_secondary, "Last saved");
                ui.label(saved);
                ui.end_row();
                ui.colored_label(p.text_secondary, "Size on disk");
                ui.label(disk);
                ui.end_row();
                ui.colored_label(p.text_secondary, "Resolution");
                ui.label("96 DPI");
                ui.end_row();
                ui.colored_label(p.text_secondary, "Layers");
                ui.label(self.doc.layers.len().to_string());
                ui.end_row();
            });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label("Units:");
                for u in [Units::Pixels, Units::Inches, Units::Centimeters] {
                    ui.radio_value(&mut s.units, u, u.label());
                }
            });
            let factor = s.units.per_pixel();
            let decimals = if s.units == Units::Pixels { 0 } else { 2 };
            let max = document::MAX_DIMENSION as f64 * factor;
            let mut w = s.width * factor;
            let mut h = s.height * factor;
            if number_row(ui, "Width", &mut w, factor..=max, decimals, "") {
                s.width = (w / factor).round().max(1.0);
            }
            if number_row(ui, "Height", &mut h, factor..=max, decimals, "") {
                s.height = (h / factor).round().max(1.0);
            }
        });
        match choice {
            Choice::Primary => {
                let (w, h) = (s.width as u32, s.height as u32);
                if (w, h) != (self.doc.width(), self.doc.height()) {
                    self.resize_canvas(w, h);
                    self.view.fit_pending = true;
                }
                false
            }
            Choice::Secondary => {
                s.width = document::DEFAULT_WIDTH as f64;
                s.height = document::DEFAULT_HEIGHT as f64;
                true
            }
            Choice::Cancel => false,
            Choice::None => true,
        }
    }

    fn edit_colors_dialog(&mut self, ctx: &egui::Context, s: &mut EditColors) -> bool {
        let p = Palette::of(ctx);
        let choice = content_dialog(ctx, "edit-colors", 420.0, "Edit colors", ("OK", None, Some("Cancel")), true, |ui| {
            // Hue × saturation spectrum.
            let (rect, response) = ui.allocate_exact_size(vec2(372.0, 220.0), Sense::click_and_drag());
            let mut mesh = Mesh::default();
            let (cols, rows) = (36, 6);
            for row in 0..=rows {
                for col in 0..=cols {
                    let h = col as f32 / cols as f32 * 360.0;
                    let sat = 1.0 - row as f32 / rows as f32;
                    let pos = pos2(rect.min.x + rect.width() * col as f32 / cols as f32, rect.min.y + rect.height() * row as f32 / rows as f32);
                    // Always shown at full brightness; brightness has its own slider below.
                    mesh.colored_vertex(pos, to_color32(Rgba::from_hsv(h, sat, 1.0)));
                }
            }
            for row in 0..rows {
                for col in 0..cols {
                    let i = (row * (cols + 1) + col) as u32;
                    let j = i + cols as u32 + 1;
                    mesh.add_triangle(i, i + 1, j);
                    mesh.add_triangle(i + 1, j + 1, j);
                }
            }
            let painter = ui.painter_at(rect);
            painter.add(mesh);
            painter.rect_stroke(rect, CornerRadius::same(4), Stroke::new(1.0, p.border), StrokeKind::Inside);
            if let Some(pos) = response.interact_pointer_pos() {
                let t = ((pos - rect.min) / rect.size()).clamp(Vec2::ZERO, Vec2::splat(1.0));
                s.hue = (t.x * 360.0).min(359.99);
                s.saturation = 1.0 - t.y;
                if s.value < 0.05 {
                    s.value = 1.0;
                }
                s.hex = s.color().to_hex();
            }
            let marker = pos2(rect.min.x + s.hue / 360.0 * rect.width(), rect.min.y + (1.0 - s.saturation) * rect.height());
            painter.circle_stroke(marker, 7.0, Stroke::new(2.0, Color32::WHITE));
            painter.circle_stroke(marker, 8.5, Stroke::new(1.0, Color32::from_black_alpha(160)));
            ui.add_space(10.0);

            // Brightness slider.
            let (track, response) = ui.allocate_exact_size(vec2(372.0, 18.0), Sense::click_and_drag());
            let mut mesh = Mesh::default();
            let top = Rgba::from_hsv(s.hue, s.saturation, 1.0);
            mesh.colored_vertex(track.left_top(), Color32::BLACK);
            mesh.colored_vertex(track.right_top(), to_color32(top));
            mesh.colored_vertex(track.right_bottom(), to_color32(top));
            mesh.colored_vertex(track.left_bottom(), Color32::BLACK);
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            ui.painter().add(mesh);
            ui.painter().rect_stroke(track, CornerRadius::same(9), Stroke::new(1.0, p.border), StrokeKind::Inside);
            if let Some(pos) = response.interact_pointer_pos() {
                s.value = ((pos.x - track.min.x) / track.width()).clamp(0.0, 1.0);
                s.hex = s.color().to_hex();
            }
            let knob = pos2(track.min.x + s.value * track.width(), track.center().y);
            ui.painter().circle(knob, 9.0, Color32::WHITE, Stroke::new(1.0, Color32::from_black_alpha(140)));
            ui.painter().circle_filled(knob, 5.0, to_color32(s.color()));
            ui.add_space(12.0);

            ui.horizontal(|ui| {
                // Old vs new preview.
                let (r, _) = ui.allocate_exact_size(vec2(64.0, 64.0), Sense::hover());
                let top = Rect::from_min_max(r.min, pos2(r.max.x, r.center().y));
                let bottom = Rect::from_min_max(pos2(r.min.x, r.center().y), r.max);
                ui.painter().rect_filled(top, CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, to_color32(s.color()));
                ui.painter().rect_filled(bottom, CornerRadius { nw: 0, ne: 0, sw: 6, se: 6 }, to_color32(s.original));
                ui.painter().rect_stroke(r, CornerRadius::same(6), Stroke::new(1.0, p.border), StrokeKind::Inside);
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.add_sized(vec2(40.0, 28.0), egui::Label::new("Hex"));
                        let r = ui.add_sized(vec2(110.0, 28.0), egui::TextEdit::singleline(&mut s.hex).char_limit(9));
                        if r.changed() {
                            if let Some(c) = Rgba::from_hex(&s.hex) {
                                let hex = s.hex.clone();
                                s.set_color(c.with_alpha(255));
                                s.hex = hex;
                            }
                        }
                    });
                    let mut c = s.color();
                    let mut changed = false;
                    ui.horizontal(|ui| {
                        for (label, v) in [("R", &mut c.r), ("G", &mut c.g), ("B", &mut c.b)] {
                            ui.label(label);
                            changed |= ui.add_sized(vec2(52.0, 28.0), egui::DragValue::new(v).range(0..=255)).changed();
                        }
                    });
                    if changed {
                        s.set_color(c);
                    }
                });
            });
        });
        match choice {
            Choice::Primary => {
                let c = s.color();
                self.set_color(s.slot, c);
                self.custom_colors.retain(|x| *x != c);
                self.custom_colors.insert(0, c);
                self.custom_colors.truncate(10);
                false
            }
            Choice::Cancel | Choice::Secondary => false,
            Choice::None => true,
        }
    }

    fn print_dialog(&mut self, ctx: &egui::Context, s: &mut Print) -> bool {
        let windows = cfg!(target_os = "windows");
        let can_print = windows || !s.printers.is_empty();
        let p = Palette::of(ctx);
        let choice = content_dialog(ctx, "print", 400.0, "Print", ("Print", None, Some("Cancel")), can_print, |ui| {
            if windows {
                ui.label("The picture will open in Windows' print dialog, where you can choose a printer and layout.");
                return;
            }
            if s.printers.is_empty() {
                ui.label("No printers were found. Add a printer in your system settings, then try again.");
                return;
            }
            ui.horizontal(|ui| {
                ui.add_sized(vec2(90.0, 28.0), egui::Label::new("Printer"));
                egui::ComboBox::from_id_salt("printer")
                    .width(220.0)
                    .selected_text(s.printers[s.printer].clone())
                    .show_ui(ui, |ui| {
                        for (i, name) in s.printers.iter().enumerate() {
                            ui.selectable_value(&mut s.printer, i, name);
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.add_sized(vec2(90.0, 28.0), egui::Label::new("Copies"));
                ui.add(egui::DragValue::new(&mut s.copies).range(1..=99));
            });
            ui.horizontal(|ui| {
                ui.add_sized(vec2(90.0, 28.0), egui::Label::new("Orientation"));
                ui.radio_value(&mut s.landscape, false, "Portrait");
                ui.radio_value(&mut s.landscape, true, "Landscape");
            });
            ui.checkbox(&mut s.fit, "Fit picture to page");
            ui.label(egui::RichText::new(format!("{} × {} px", self.doc.width(), self.doc.height())).font(theme::caption()).color(p.text_secondary));
        });
        match choice {
            Choice::Primary => {
                let printer = s.printers.get(s.printer).cloned();
                self.print(printer, s.copies, s.fit, s.landscape);
                false
            }
            Choice::Cancel | Choice::Secondary => false,
            Choice::None => true,
        }
    }
}

/// "YYYY-MM-DD HH:MM" (UTC) without pulling in a date library.
fn format_timestamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let (h, m) = ((secs % 86_400) / 3600, (secs % 3600) / 60);
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    format!("{year:04}-{month:02}-{d:02} {h:02}:{m:02} UTC")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps() {
        assert_eq!(format_timestamp(0), "1970-01-01 00:00 UTC");
        assert_eq!(format_timestamp(1_700_000_000), "2023-11-14 22:13 UTC");
    }

    #[test]
    fn sizes() {
        assert_eq!(format_size(512), "512 bytes");
        assert_eq!(format_size(2048), "2.0 KB");
        assert_eq!(format_size(3 * 1024 * 1024), "3.0 MB");
    }

    #[test]
    fn edit_colors_keeps_hue_for_greys() {
        let mut s = EditColors {
            hue: 200.0,
            saturation: 0.5,
            value: 0.5,
            hex: String::new(),
            slot: ColorSlot::Primary,
            original: Rgba::BLACK,
        };
        s.set_color(Rgba::BLACK);
        assert_eq!(s.hue, 200.0);
        assert_eq!(s.value, 0.0);
        s.set_color(Rgba::rgb(255, 0, 0));
        assert_eq!((s.hue, s.saturation, s.value), (0.0, 1.0, 1.0));
    }

    #[test]
    fn units() {
        assert!((Units::Inches.per_pixel() * 96.0 - 1.0).abs() < 1e-9);
        assert!((Units::Centimeters.per_pixel() * 96.0 - 2.54).abs() < 1e-9);
    }
}
