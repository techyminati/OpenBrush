// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! The Layers panel.

use std::collections::HashMap;

use egui::{Align, Align2, Color32, CornerRadius, Layout, Popup, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Ui, pos2, vec2};

use super::commands::Command;
use super::PaintApp;
use crate::engine::document::{BlendMode, LayerId};
use crate::engine::transform;
use crate::ui::icons::Icon;
use crate::ui::theme::{self, Palette};
use crate::ui::widgets::{self, MenuItem, menu_separator};

const THUMB: egui::Vec2 = egui::vec2(120.0, 72.0);

/// Layer thumbnails, refreshed whenever the document changes.
#[derive(Default)]
pub(crate) struct Thumbnails {
    textures: HashMap<LayerId, TextureHandle>,
    version: Option<(u64, usize, u32, u32)>,
}

impl PaintApp {
    fn refresh_thumbnails(&mut self, ctx: &egui::Context) {
        let version = (self.history.state_id(), self.doc.layers.len(), self.doc.width(), self.doc.height());
        if self.thumbnails.version == Some(version) {
            return;
        }
        self.thumbnails.version = Some(version);
        let scale = (THUMB.x * 2.0 / self.doc.width() as f32).min(THUMB.y * 2.0 / self.doc.height() as f32).min(1.0);
        let (w, h) = (((self.doc.width() as f32 * scale).round() as u32).max(1), ((self.doc.height() as f32 * scale).round() as u32).max(1));
        let mut textures = HashMap::new();
        for layer in &self.doc.layers {
            let small = transform::scale(&layer.pixels, w, h, true);
            let image = egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], small.data());
            let texture = match self.thumbnails.textures.remove(&layer.id) {
                Some(mut t) => {
                    t.set(image, TextureOptions::LINEAR);
                    t
                }
                None => ctx.load_texture(format!("layer-{}", layer.id), image, TextureOptions::LINEAR),
            };
            textures.insert(layer.id, texture);
        }
        self.thumbnails.textures = textures;
    }

    pub(crate) fn layers_panel(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.refresh_thumbnails(&ctx);
        let p = Palette::of(&ctx);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Layers").font(theme::semibold(16.0)));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::icon_button(ui, Icon::Dismiss, vec2(28.0, 28.0), false, true, "Close layers").clicked() {
                    self.execute(&ctx, Command::ToggleLayers);
                }
                if widgets::icon_button(ui, Icon::Add, vec2(28.0, 28.0), false, true, "Add layer").clicked() {
                    self.execute(&ctx, Command::AddLayer);
                }
            });
        });
        ui.add_space(6.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            for index in (0..self.doc.layers.len()).rev() {
                self.layer_card(ui, index, &p);
            }
        });
    }

    fn layer_card(&mut self, ui: &mut Ui, index: usize, p: &Palette) {
        let ctx = ui.ctx().clone();
        let width = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(vec2(width, THUMB.y + 16.0), Sense::click());
        let layer = &self.doc.layers[index];
        let active = index == self.doc.active;
        let fill = if response.hovered() { p.surface } else { p.surface.gamma_multiply(if p.dark { 0.85 } else { 0.7 }) };
        ui.painter().rect(
            rect,
            CornerRadius::same(6),
            fill,
            if active { Stroke::new(2.0, p.accent) } else { Stroke::new(1.0, p.border) },
            StrokeKind::Inside,
        );
        // Thumbnail, letterboxed on a checkerboard.
        let box_rect = Rect::from_min_size(rect.min + vec2(8.0, 8.0), THUMB);
        let aspect = self.doc.width() as f32 / self.doc.height() as f32;
        let size = if aspect > THUMB.x / THUMB.y { vec2(THUMB.x, THUMB.x / aspect) } else { vec2(THUMB.y * aspect, THUMB.y) };
        let thumb = Rect::from_center_size(box_rect.center(), size);
        widgets::checkerboard(ui.painter(), thumb, 5.0);
        if let Some(tex) = self.thumbnails.textures.get(&layer.id) {
            let tint = if layer.visible { Color32::WHITE } else { Color32::from_white_alpha(90) };
            ui.painter().image(tex.id(), thumb, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
        }
        ui.painter().rect_stroke(thumb, 0.0, Stroke::new(1.0, p.border), StrokeKind::Outside);
        let text_x = box_rect.max.x + 10.0;
        ui.painter().text(pos2(text_x, rect.min.y + 18.0), Align2::LEFT_CENTER, &layer.name, theme::body(), p.text);
        let detail = if layer.blend == BlendMode::Normal && layer.opacity >= 1.0 {
            String::new()
        } else {
            format!("{} · {}%", layer.blend.label(), (layer.opacity * 100.0).round())
        };
        ui.painter().text(pos2(text_x, rect.min.y + 36.0), Align2::LEFT_CENTER, detail, theme::caption(), p.text_secondary);
        let name = layer.name.clone();
        let visible = layer.visible;
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name.clone()));
        if response.clicked() {
            self.select_layer(index);
        }

        // Visibility and "more" buttons.
        let buttons = Rect::from_min_size(pos2(text_x - 4.0, rect.max.y - 36.0), vec2(rect.max.x - text_x - 4.0, 30.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(buttons).layout(Layout::left_to_right(Align::Center)));
        child.spacing_mut().item_spacing.x = 2.0;
        let eye = if visible { Icon::Eye } else { Icon::EyeOff };
        if widgets::icon_button(&mut child, eye, vec2(30.0, 28.0), false, true, if visible { "Hide layer" } else { "Show layer" }).clicked() {
            self.set_layer_visible(index, !visible);
        }
        let more = widgets::icon_button(&mut child, Icon::More, vec2(30.0, 28.0), false, true, "Layer options");
        let frame = widgets::popup_frame(&child);
        Popup::menu(&more)
            .frame(frame)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_min_width(250.0);
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                let layers = self.doc.layers.len();
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label("Opacity");
                    let mut opacity = self.doc.layers[index].opacity * 100.0;
                    let r = ui.add(egui::Slider::new(&mut opacity, 0.0..=100.0).suffix("%").max_decimals(0));
                    if r.drag_started() {
                        self.opacity_before = Some(self.doc.clone());
                    }
                    if r.changed() {
                        // A drag is recorded once when it ends; a click is recorded right away.
                        if self.opacity_before.is_none() {
                            let before = self.doc.clone();
                            self.set_layer_opacity(index, opacity / 100.0);
                            self.history.push_document("Layer opacity", before);
                        } else {
                            self.set_layer_opacity(index, opacity / 100.0);
                        }
                    }
                    if r.drag_stopped() {
                        if let Some(before) = self.opacity_before.take() {
                            self.history.push_document("Layer opacity", before);
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label("Blend mode");
                    let mut blend = self.doc.layers[index].blend;
                    egui::ComboBox::from_id_salt(("blend", index)).selected_text(blend.label()).width(130.0).show_ui(ui, |ui| {
                        for mode in BlendMode::ALL {
                            ui.selectable_value(&mut blend, mode, mode.label());
                        }
                    });
                    if blend != self.doc.layers[index].blend {
                        self.set_layer_blend(index, blend);
                    }
                });
                ui.add_space(4.0);
                menu_separator(ui);
                let visible = self.doc.layers[index].visible;
                if MenuItem::new(if visible { "Hide layer" } else { "Show layer" })
                    .icon(if visible { Icon::EyeOff } else { Icon::Eye })
                    .show(ui)
                    .clicked()
                {
                    self.set_layer_visible(index, !visible);
                    ui.close();
                }
                if MenuItem::new("Duplicate layer").icon(Icon::Duplicate).show(ui).clicked() {
                    self.duplicate_layer(index);
                    ui.close();
                }
                if MenuItem::new("Merge down").icon(Icon::MergeDown).enabled(index > 0).show(ui).clicked() {
                    self.merge_down(index);
                    ui.close();
                }
                if MenuItem::new("Move up").icon(Icon::MoveUp).enabled(index + 1 < layers).show(ui).clicked() {
                    self.move_layer(index, index + 1);
                    ui.close();
                }
                if MenuItem::new("Move down").icon(Icon::MoveDown).enabled(index > 0).show(ui).clicked() {
                    self.move_layer(index, index - 1);
                    ui.close();
                }
                menu_separator(ui);
                if MenuItem::new("Delete layer").icon(Icon::Delete).enabled(layers > 1).show(ui).clicked() {
                    self.delete_layer(index);
                    ui.close();
                }
            });
        let _ = ctx;
    }
}
