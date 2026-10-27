// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! The status bar: pointer position, selection and canvas size, file size and zoom.

use egui::{Align, Align2, Color32, Layout, Popup, Pos2, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2};

use super::commands::Command;
use super::dialogs::format_file_size;
use super::state::{MAX_ZOOM, MIN_ZOOM, ZOOM_LEVELS};
use super::tools::Gesture;
use super::PaintApp;
use crate::ui::icons::Icon;
use crate::ui::theme::{self, Palette};
use crate::ui::widgets::{self, MenuItem};

fn zoom_to_t(zoom: f32) -> f32 {
    (zoom.ln() - MIN_ZOOM.ln()) / (MAX_ZOOM.ln() - MIN_ZOOM.ln())
}

fn t_to_zoom(t: f32) -> f32 {
    (MIN_ZOOM.ln() + t.clamp(0.0, 1.0) * (MAX_ZOOM.ln() - MIN_ZOOM.ln())).exp()
}

fn status_item(ui: &mut Ui, icon: Icon, text: &str, width: f32) {
    let p = Palette::of(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(vec2(width, 24.0), Sense::hover());
    icon.paint(ui, Rect::from_min_size(rect.min + vec2(0.0, 3.0), Vec2::splat(18.0)), 16.0, p.text_secondary);
    ui.painter().text(pos2(rect.min.x + 24.0, rect.center().y), Align2::LEFT_CENTER, text, theme::caption(), p.text);
}

impl PaintApp {
    pub(crate) fn status_bar(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui.ctx());
        ui.horizontal(|ui| {
            ui.set_height(28.0);
            ui.spacing_mut().item_spacing.x = 6.0;
            let pos = self
                .cursor_pixel
                .filter(|(x, y)| *x >= 0 && *y >= 0 && (*x as u32) < self.doc.width() && (*y as u32) < self.doc.height())
                .map(|(x, y)| format!("{x}, {y}px"))
                .unwrap_or_default();
            status_item(ui, Icon::Cursor, &pos, 120.0);
            let selection = match &self.gesture {
                Gesture::Marquee { start, end } => Some(((end.x - start.x).abs().round(), (end.y - start.y).abs().round())),
                _ => self.selection.as_ref().map(|s| {
                    let b = s.sel.bounds();
                    (b.width().round(), b.height().round())
                }),
            };
            let selection = selection.map(|(w, h)| format!("{w} × {h}px")).unwrap_or_default();
            status_item(ui, Icon::SelectionSize, &selection, 120.0);
            let (w, h) = match &self.gesture {
                Gesture::ResizeCanvas { width, height, .. } => (*width, *height),
                _ => (self.doc.width(), self.doc.height()),
            };
            status_item(ui, Icon::CanvasSize, &format!("{w} × {h}px"), 130.0);
            if let Some(size) = self.file.size_on_disk {
                status_item(ui, Icon::FileSize, &format!("Size: {}", format_file_size(size)), 140.0);
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let ctx = ui.ctx().clone();
                let viewport = self.viewport;
                if widgets::icon_button(ui, Icon::ZoomFit, vec2(28.0, 26.0), false, true, "Fit to window").clicked() {
                    self.execute(&ctx, Command::ZoomFit);
                }
                if widgets::icon_button(ui, Icon::ZoomIn, vec2(28.0, 26.0), false, true, "Zoom in").clicked() {
                    self.execute(&ctx, Command::ZoomIn);
                }
                // A Fluent-style slider with a logarithmic scale.
                let (track, response) = ui.allocate_exact_size(vec2(130.0, 26.0), Sense::click_and_drag());
                let line = Rect::from_center_size(track.center(), vec2(track.width() - 12.0, 4.0));
                let t = zoom_to_t(self.view.zoom);
                let knob = pos2(line.min.x + t * line.width(), line.center().y);
                ui.painter().rect_filled(line, 2.0, p.text_secondary.gamma_multiply(0.45));
                ui.painter().rect_filled(Rect::from_min_max(line.min, pos2(knob.x, line.max.y)), 2.0, p.accent);
                ui.painter().circle(knob, 9.0, p.surface, Stroke::new(1.0, p.border));
                let inner = if response.hovered() || response.dragged() { 6.0 } else { 5.0 };
                ui.painter().circle_filled(knob, inner, p.accent);
                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Slider, true, "Zoom"));
                if let Some(pos) = response.interact_pointer_pos() {
                    let t = (pos.x - line.min.x) / line.width();
                    let mut z = t_to_zoom(t);
                    // Snap to the presets when close.
                    if let Some(snap) = ZOOM_LEVELS.iter().find(|&&l| (zoom_to_t(l) - zoom_to_t(z)).abs() < 0.015) {
                        z = *snap;
                    }
                    self.zoom_to(z, None, viewport);
                }
                if widgets::icon_button(ui, Icon::ZoomOut, vec2(28.0, 26.0), false, true, "Zoom out").clicked() {
                    self.execute(&ctx, Command::ZoomOut);
                }
                let label = format!("{}%", (self.view.zoom * 100.0).round());
                let response = widgets::menu_bar_button(ui, &label, false);
                let frame = widgets::popup_frame(ui);
                Popup::menu(&response).frame(frame).align(egui::RectAlign::TOP_START).show(|ui| {
                    ui.set_min_width(160.0);
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    for &z in ZOOM_LEVELS.iter().rev() {
                        let text = format!("{}%", z * 100.0);
                        let checked = (self.view.zoom - z).abs() < 1e-4;
                        if MenuItem::new(&text).checked(checked).show(ui).clicked() {
                            self.zoom_to(z, None, viewport);
                        }
                    }
                    widgets::menu_separator(ui);
                    if MenuItem::new("Fit to window").show(ui).clicked() {
                        self.execute(&ctx, Command::ZoomFit);
                    }
                });
            });
        });
        let _ = (Color32::TRANSPARENT, Pos2::ZERO);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_mapping_round_trips() {
        for z in ZOOM_LEVELS {
            assert!((t_to_zoom(zoom_to_t(z)) - z).abs() < 1e-3);
        }
        assert_eq!(t_to_zoom(-1.0), MIN_ZOOM);
        assert!((t_to_zoom(2.0) - MAX_ZOOM).abs() < 1e-4);
    }
}
