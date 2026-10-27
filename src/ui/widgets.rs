// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Small custom-painted widgets in the Windows 11 Fluent style.

use egui::{Align2, Color32, CornerRadius, Frame, Margin, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Role, WidgetInfo, vec2};

use super::icons::Icon;
use super::theme::{self, Palette};
use crate::engine::color::Rgba;

pub fn to_color32(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

fn label_widget(response: &Response, kind: Role, enabled: bool, label: &str) {
    let label = label.to_owned();
    response.widget_info(move || WidgetInfo::labeled(kind, enabled, label.clone()));
}

/// Background for a subtle (borderless) button in its current state.
fn subtle_fill(p: &Palette, response: &Response, selected: bool, enabled: bool) -> Color32 {
    if selected {
        p.selected
    } else if !enabled {
        Color32::TRANSPARENT
    } else if response.is_pointer_button_down_on() {
        p.pressed
    } else if response.hovered() {
        p.hover
    } else {
        Color32::TRANSPARENT
    }
}

/// A square, borderless icon button, as used throughout Paint's toolbar.
pub fn icon_button(ui: &mut Ui, icon: Icon, size: Vec2, selected: bool, enabled: bool, label: &str) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let p = Palette::of(ui.ctx());
    if ui.is_rect_visible(rect) {
        let fill = subtle_fill(&p, &response, selected, enabled);
        ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
        let color = if enabled { p.icon } else { p.text_disabled };
        icon.paint(ui, rect, 20.0, color);
    }
    label_widget(&response, Role::Button, enabled, label);
    response.on_hover_text(label)
}

/// A taller button with an icon and a dropdown chevron underneath (Select, Brushes, Size…).
pub fn dropdown_button(ui: &mut Ui, icon: Icon, size: Vec2, selected: bool, open: bool, label: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let p = Palette::of(ui.ctx());
    if ui.is_rect_visible(rect) {
        let fill = if open { p.pressed } else { subtle_fill(&p, &response, selected, true) };
        ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
        let icon_rect = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.max.y - 14.0));
        icon.paint(ui, icon_rect, 24.0, p.icon);
        let chevron = Rect::from_center_size(Pos2::new(rect.center().x, rect.max.y - 10.0), vec2(12.0, 12.0));
        Icon::ChevronDown.paint(ui, chevron, 10.0, p.text_secondary);
    }
    label_widget(&response, Role::Button, true, label);
    response.on_hover_text(label)
}

/// A wide button with an icon, a label and a chevron (Outline ▾, Fill ▾).
pub fn labeled_dropdown(ui: &mut Ui, icon: Icon, label: &str, width: f32, enabled: bool) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(vec2(width, 28.0), sense);
    let p = Palette::of(ui.ctx());
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, CornerRadius::same(4), subtle_fill(&p, &response, false, enabled));
        let color = if enabled { p.icon } else { p.text_disabled };
        icon.paint(ui, Rect::from_min_size(rect.min + vec2(4.0, 4.0), vec2(20.0, 20.0)), 18.0, color);
        ui.painter().text(
            Pos2::new(rect.min.x + 28.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            theme::caption(),
            if enabled { p.text } else { p.text_disabled },
        );
        Icon::ChevronDown.paint(
            ui,
            Rect::from_center_size(Pos2::new(rect.max.x - 10.0, rect.center().y), vec2(12.0, 12.0)),
            10.0,
            p.text_secondary,
        );
    }
    label_widget(&response, Role::Button, enabled, label);
    response
}

/// Text-only button for the menu bar ("File", "Edit", "View").
pub fn menu_bar_button(ui: &mut Ui, label: &str, open: bool) -> Response {
    let galley = ui.painter().layout_no_wrap(label.to_owned(), theme::body(), Color32::PLACEHOLDER);
    let size = vec2(galley.size().x + 22.0, 30.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let p = Palette::of(ui.ctx());
    if ui.is_rect_visible(rect) {
        let fill = if open { p.pressed } else { subtle_fill(&p, &response, false, true) };
        ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
        ui.painter().galley(rect.center() - galley.size() / 2.0, galley, p.text);
    }
    label_widget(&response, Role::Button, true, label);
    response
}

/// One row in a dropdown menu.
pub struct MenuItem<'a> {
    pub icon: Option<Icon>,
    pub label: &'a str,
    pub shortcut: Option<&'a str>,
    pub enabled: bool,
    pub checked: Option<bool>,
    pub submenu: bool,
}

impl<'a> MenuItem<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            icon: None,
            label,
            shortcut: None,
            enabled: true,
            checked: None,
            submenu: false,
        }
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn shortcut(mut self, shortcut: &'a str) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    pub fn submenu(mut self) -> Self {
        self.submenu = true;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let width = ui.available_width().max(240.0);
        let sense = if self.enabled { Sense::click() } else { Sense::hover() };
        let (rect, response) = ui.allocate_exact_size(vec2(width, 34.0), sense);
        let p = Palette::of(ui.ctx());
        if ui.is_rect_visible(rect) {
            let inner = rect.shrink2(vec2(4.0, 1.0));
            let highlight = self.enabled && (response.hovered() || (self.submenu && response.is_pointer_button_down_on()));
            if highlight {
                ui.painter().rect_filled(inner, CornerRadius::same(4), p.hover);
            }
            let text_color = if self.enabled { p.text } else { p.text_disabled };
            let mut x = inner.min.x + 12.0;
            if let Some(checked) = self.checked {
                if checked {
                    Icon::Checkmark.paint(ui, Rect::from_min_size(Pos2::new(x, inner.center().y - 8.0), vec2(16.0, 16.0)), 16.0, text_color);
                }
                x += 26.0;
            }
            if let Some(icon) = self.icon {
                icon.paint(ui, Rect::from_min_size(Pos2::new(x, inner.center().y - 10.0), vec2(20.0, 20.0)), 18.0, text_color);
                x += 30.0;
            } else if self.checked.is_none() {
                x += 30.0;
            }
            ui.painter().text(Pos2::new(x, inner.center().y), Align2::LEFT_CENTER, self.label, theme::body(), text_color);
            if let Some(shortcut) = self.shortcut {
                ui.painter().text(
                    Pos2::new(inner.max.x - 12.0, inner.center().y),
                    Align2::RIGHT_CENTER,
                    shortcut,
                    theme::caption(),
                    if self.enabled { p.text_secondary } else { p.text_disabled },
                );
            }
            if self.submenu {
                Icon::ChevronRight.paint(
                    ui,
                    Rect::from_center_size(Pos2::new(inner.max.x - 14.0, inner.center().y), vec2(12.0, 12.0)),
                    10.0,
                    text_color,
                );
            }
        }
        label_widget(
            &response,
            if self.checked.is_some() { Role::CheckBox } else { Role::Button },
            self.enabled,
            self.label,
        );
        response
    }
}

pub fn menu_separator(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 9.0), Sense::hover());
    let p = Palette::of(ui.ctx());
    ui.painter().hline(rect.x_range().shrink(2.0), rect.center().y, Stroke::new(1.0, p.divider));
}

pub fn vertical_divider(ui: &mut Ui, height: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, height), Sense::hover());
    let p = Palette::of(ui.ctx());
    ui.painter().vline(rect.center().x, rect.y_range().shrink(6.0), Stroke::new(1.0, p.divider));
}

/// Frame for menus and popups: rounded, bordered and shadowed.
pub fn popup_frame(ui: &Ui) -> Frame {
    let p = Palette::of(ui.ctx());
    Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(4))
        .shadow(ui.visuals().popup_shadow)
}

/// Accent ("primary") or standard dialog button.
pub fn dialog_button(ui: &mut Ui, label: &str, primary: bool, enabled: bool) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), theme::body(), Color32::PLACEHOLDER);
    let width = (galley.size().x + 32.0).max(96.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 32.0), sense);
    let p = Palette::of(ui.ctx());
    if ui.is_rect_visible(rect) {
        let (fill, text, stroke) = if primary {
            let base = if enabled { p.accent } else { p.text_disabled };
            let fill = if response.is_pointer_button_down_on() {
                base.gamma_multiply(0.8)
            } else if response.hovered() {
                base.gamma_multiply(0.9)
            } else {
                base
            };
            (fill, p.accent_text, Stroke::NONE)
        } else {
            let fill = ui.visuals().widgets.inactive.weak_bg_fill;
            let fill = if response.hovered() && enabled { ui.visuals().widgets.hovered.weak_bg_fill } else { fill };
            (fill, if enabled { p.text } else { p.text_disabled }, Stroke::new(1.0, p.border))
        };
        ui.painter().rect(rect, CornerRadius::same(4), fill, stroke, StrokeKind::Inside);
        ui.painter().galley(rect.center() - galley.size() / 2.0, galley, text);
    }
    label_widget(&response, Role::Button, enabled, label);
    response
}

/// Draws a light/dark checkerboard, used to show transparency.
pub fn checkerboard(painter: &egui::Painter, rect: Rect, cell: f32) {
    painter.rect_filled(rect, 0.0, Color32::WHITE);
    let grey = Color32::from_rgb(0xE3, 0xE3, 0xE3);
    let cols = (rect.width() / cell).ceil() as i32;
    let rows = (rect.height() / cell).ceil() as i32;
    let mut mesh = egui::Mesh::default();
    for row in 0..rows {
        for col in 0..cols {
            if (row + col) % 2 == 0 {
                continue;
            }
            let min = rect.min + vec2(col as f32 * cell, row as f32 * cell);
            let r = Rect::from_min_size(min, Vec2::splat(cell)).intersect(rect);
            mesh.add_colored_rect(r, grey);
        }
    }
    painter.add(mesh);
}

/// A round colour swatch, as in Paint's palette. `None` draws an empty custom-colour slot.
pub fn color_circle(ui: &mut Ui, color: Option<Rgba>, diameter: f32, label: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(diameter + 4.0), Sense::click());
    let p = Palette::of(ui.ctx());
    if ui.is_rect_visible(rect) {
        let center = rect.center();
        let r = diameter / 2.0;
        match color {
            Some(c) => {
                ui.painter().circle_filled(center, r, to_color32(c));
                ui.painter().circle_stroke(center, r, Stroke::new(1.0, p.border.gamma_multiply(1.4)));
            }
            None => {
                ui.painter().circle_stroke(center, r - 0.5, Stroke::new(1.0, p.border.gamma_multiply(1.6)));
            }
        }
        if response.hovered() && color.is_some() {
            ui.painter().circle_stroke(center, r + 1.5, Stroke::new(1.5, p.text_secondary));
        }
    }
    label_widget(&response, Role::Button, true, label);
    response.on_hover_text(label)
}

/// A rainbow wheel for the "Edit colors" button.
pub fn paint_hue_wheel(painter: &egui::Painter, center: Pos2, radius: f32) {
    let mut mesh = egui::Mesh::default();
    let segments = 48;
    mesh.colored_vertex(center, Color32::WHITE);
    for i in 0..=segments {
        let t = i as f32 / segments as f32;
        let angle = t * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let c = Rgba::from_hsv(t * 360.0, 1.0, 1.0);
        mesh.colored_vertex(center + vec2(angle.cos(), angle.sin()) * radius, to_color32(c));
        if i > 0 {
            mesh.add_triangle(0, i, i + 1);
        }
    }
    painter.add(mesh);
}
