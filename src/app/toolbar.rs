// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! The toolbar: Selection, Image, Tools, Brushes, Shapes, Size, Colors and Layers.

use egui::{Align2, Color32, CornerRadius, Popup, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Ui, Vec2, pos2, vec2};
use tiny_skia::Point;

use super::commands::{Command, cmd, cmd_shift};
use super::state::{ColorSlot, SelectMode, SizeKind, Tool};
use super::PaintApp;
use crate::engine::brush::{BrushKind, Stroke as BrushStroke, StrokeKind as BrushStrokeKind, apply_draft};
use crate::engine::color::{DEFAULT_PALETTE, Rgba};
use crate::engine::raster;
use crate::engine::shapes::{self, Geometry, PaintStyle, ShapeKind, ShapeStyle};
use crate::ui::icons::Icon;
use crate::ui::theme::{self, Palette};
use crate::ui::widgets::{self, MenuItem, menu_separator, to_color32};

const ROW_H: f32 = 66.0;
const LABEL_H: f32 = 18.0;

/// Preview images for the brush and outline/fill dropdowns, rendered with the real engine.
#[derive(Default)]
pub(crate) struct Previews {
    brushes: Vec<(BrushKind, TextureHandle)>,
    styles: Vec<(PaintStyle, TextureHandle)>,
}

fn preview_texture(ctx: &egui::Context, name: &str, pixmap: &tiny_skia::Pixmap) -> TextureHandle {
    let image = egui::ColorImage::from_rgba_premultiplied([pixmap.width() as usize, pixmap.height() as usize], pixmap.data());
    ctx.load_texture(name, image, TextureOptions::LINEAR)
}

impl Previews {
    fn ensure(&mut self, ctx: &egui::Context) {
        if !self.brushes.is_empty() {
            return;
        }
        // Rendered in white at 2× and tinted by the theme's text colour when drawn.
        for (i, kind) in BrushKind::ALL.into_iter().enumerate() {
            let mut canvas = raster::new_pixmap(240, 56);
            let mut draft = raster::new_pixmap(240, 56);
            let mut stroke = BrushStroke::new(BrushStrokeKind::Brush(kind), Rgba::WHITE, 12.0, i as u64 + 7);
            let mut dirty = None;
            for step in 0..=60 {
                let x = 16.0 + step as f32 * 3.5;
                let y = 28.0 - (step as f32 / 60.0 * std::f32::consts::TAU).sin() * 12.0;
                dirty = raster::union_opt(dirty, stroke.add_point(&mut draft, x, y));
                if step % 6 == 0 {
                    dirty = raster::union_opt(dirty, stroke.tick(&mut draft, 0.03));
                }
            }
            if let Some(d) = dirty {
                apply_draft(&mut canvas, &draft, d, &stroke.composite_paint());
            }
            self.brushes.push((kind, preview_texture(ctx, &format!("brush-{i}"), &canvas)));
        }
        for (i, style) in PaintStyle::ALL.into_iter().enumerate().skip(2) {
            let mut draft = raster::new_pixmap(240, 56);
            let g = Geometry::Line {
                a: Point::from_xy(16.0, 28.0),
                b: Point::from_xy(224.0, 28.0),
            };
            let s = ShapeStyle {
                outline: style,
                fill: PaintStyle::None,
                outline_color: Rgba::WHITE,
                fill_color: Rgba::WHITE,
                width: 14.0,
            };
            shapes::render(&mut draft, &g, &s, i as u64);
            self.styles.push((style, preview_texture(ctx, &format!("style-{i}"), &draft)));
        }
    }

    fn brush(&self, kind: BrushKind) -> Option<&TextureHandle> {
        self.brushes.iter().find(|(k, _)| *k == kind).map(|(_, t)| t)
    }

    fn style(&self, style: PaintStyle) -> Option<&TextureHandle> {
        self.styles.iter().find(|(k, _)| *k == style).map(|(_, t)| t)
    }
}

/// A labelled toolbar group: content on top, caption underneath.
fn group(ui: &mut Ui, label: &str, content: impl FnOnce(&mut Ui)) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
        let width = ui
            .horizontal(|ui| {
                ui.set_height(ROW_H);
                content(ui);
            })
            .response
            .rect
            .width();
        let (rect, _) = ui.allocate_exact_size(vec2(width, LABEL_H), Sense::hover());
        let p = Palette::of(ui.ctx());
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, theme::caption(), p.text_secondary);
    });
}

/// Paints a small icon of a gallery shape, using the real shape geometry.
fn paint_shape_icon(ui: &Ui, kind: ShapeKind, rect: Rect, color: Color32) {
    let r = rect.shrink(4.0);
    let (a, b) = (Point::from_xy(r.min.x, r.min.y), Point::from_xy(r.max.x, r.max.y));
    let geometry = match kind {
        ShapeKind::Line => Geometry::Line { a, b },
        ShapeKind::Curve => Geometry::Curve {
            a: Point::from_xy(r.min.x, r.max.y),
            b: Point::from_xy(r.max.x, r.min.y),
            c1: Point::from_xy(r.min.x + r.width() * 0.2, r.min.y - r.height() * 0.4),
            c2: Point::from_xy(r.max.x - r.width() * 0.2, r.max.y + r.height() * 0.4),
        },
        ShapeKind::Polygon => Geometry::Polygon {
            points: vec![
                Point::from_xy(r.min.x, r.min.y + r.height() * 0.3),
                Point::from_xy(r.min.x + r.width() * 0.6, r.min.y),
                Point::from_xy(r.max.x, r.max.y),
                Point::from_xy(r.min.x + r.width() * 0.2, r.max.y),
            ],
            closed: true,
        },
        kind => Geometry::Boxed { kind, a, b },
    };
    let Some(path) = geometry.path() else {
        return;
    };
    for (points, closed) in shapes::flatten(&path) {
        let pts: Vec<Pos2> = points.iter().map(|p| pos2(p.x, p.y)).collect();
        let stroke = Stroke::new(1.2, color);
        if closed {
            ui.painter().add(egui::Shape::closed_line(pts, stroke));
        } else {
            ui.painter().add(egui::Shape::line(pts, stroke));
        }
    }
}

impl PaintApp {
    pub(crate) fn toolbar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.previews.ensure(&ctx);
        egui::ScrollArea::horizontal()
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    self.selection_group(ui);
                    widgets::vertical_divider(ui, ROW_H + LABEL_H);
                    self.image_group(ui);
                    widgets::vertical_divider(ui, ROW_H + LABEL_H);
                    self.tools_group(ui);
                    widgets::vertical_divider(ui, ROW_H + LABEL_H);
                    self.brushes_group(ui);
                    widgets::vertical_divider(ui, ROW_H + LABEL_H);
                    self.shapes_group(ui);
                    widgets::vertical_divider(ui, ROW_H + LABEL_H);
                    self.size_group(ui);
                    widgets::vertical_divider(ui, ROW_H + LABEL_H);
                    self.colors_group(ui);
                    widgets::vertical_divider(ui, ROW_H + LABEL_H);
                    self.layers_group(ui);
                });
            });
    }

    fn run(&mut self, ui: &Ui, command: Command) {
        let ctx = ui.ctx().clone();
        self.execute(&ctx, command);
    }

    fn selection_group(&mut self, ui: &mut Ui) {
        group(ui, "Selection", |ui| {
            let icon = if self.select_mode == SelectMode::FreeForm { Icon::SelectFreeform } else { Icon::SelectRect };
            let open = Popup::is_id_open(ui.ctx(), ui.next_auto_id().with("popup"));
            let response = widgets::dropdown_button(ui, icon, vec2(48.0, ROW_H), self.tool == Tool::Select, open, "Select");
            let frame = widgets::popup_frame(ui);
            Popup::menu(&response).frame(frame).show(|ui| {
                ui.set_min_width(260.0);
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                for (mode, label, icon) in [
                    (SelectMode::Rectangle, "Rectangle", Icon::SelectRect),
                    (SelectMode::FreeForm, "Free-form", Icon::SelectFreeform),
                ] {
                    let checked = self.select_mode == mode && self.tool == Tool::Select;
                    if MenuItem::new(label).icon(icon).checked(checked).show(ui).clicked() {
                        self.select_mode = mode;
                        self.set_tool(Tool::Select);
                    }
                }
                menu_separator(ui);
                for (label, icon, shortcut, command) in [
                    ("Select all", Icon::SelectAll, cmd("A"), Command::SelectAll),
                    ("Invert selection", Icon::InvertSelection, String::new(), Command::InvertSelection),
                    ("Delete", Icon::Delete, "Del".to_owned(), Command::DeleteSelection),
                ] {
                    let enabled = self.can_execute(&command);
                    if MenuItem::new(label).icon(icon).shortcut(&shortcut).enabled(enabled).show(ui).clicked() {
                        self.run(ui, command);
                    }
                }
                menu_separator(ui);
                let transparent = self.transparent_selection;
                if MenuItem::new("Transparent selection").checked(transparent).show(ui).clicked() {
                    self.run(ui, Command::ToggleTransparentSelection);
                }
            });
        });
    }

    fn image_group(&mut self, ui: &mut Ui) {
        group(ui, "Image", |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                ui.add_space(1.0);
                ui.horizontal(|ui| {
                    let can_crop = self.can_execute(&Command::Crop);
                    if widgets::icon_button(ui, Icon::Crop, vec2(32.0, 30.0), false, can_crop, &format!("Crop ({})", cmd_shift("X"))).clicked() {
                        self.run(ui, Command::Crop);
                    }
                    if widgets::icon_button(ui, Icon::Resize, vec2(32.0, 30.0), false, true, &format!("Resize and skew ({})", cmd("W"))).clicked() {
                        self.run(ui, Command::ResizeSkew);
                    }
                });
                ui.horizontal(|ui| {
                    let response = widgets::icon_button(ui, Icon::Rotate, vec2(66.0, 30.0), false, true, "Rotate");
                    let chevron = Rect::from_center_size(response.rect.right_center() - vec2(10.0, 0.0), vec2(10.0, 10.0));
                    Icon::ChevronDown.paint(ui, chevron, 10.0, Palette::of(ui.ctx()).text_secondary);
                    let frame = widgets::popup_frame(ui);
                    Popup::menu(&response).frame(frame).show(|ui| {
                        ui.set_min_width(230.0);
                        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                        for (label, icon, command) in [
                            ("Rotate right 90°", Icon::RotateRight, Command::RotateRight),
                            ("Rotate left 90°", Icon::RotateLeft, Command::RotateLeft),
                            ("Rotate 180°", Icon::Rotate180, Command::Rotate180),
                        ] {
                            if MenuItem::new(label).icon(icon).show(ui).clicked() {
                                self.run(ui, command);
                            }
                        }
                        menu_separator(ui);
                        for (label, icon, command) in [
                            ("Flip vertical", Icon::FlipVertical, Command::FlipVertical),
                            ("Flip horizontal", Icon::FlipHorizontal, Command::FlipHorizontal),
                        ] {
                            if MenuItem::new(label).icon(icon).show(ui).clicked() {
                                self.run(ui, command);
                            }
                        }
                    });
                });
            });
        });
    }

    fn tools_group(&mut self, ui: &mut Ui) {
        group(ui, "Tools", |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                ui.add_space(1.0);
                for row in [[Tool::Pencil, Tool::Fill, Tool::Text], [Tool::Eraser, Tool::ColorPicker, Tool::Magnifier]] {
                    ui.horizontal(|ui| {
                        for tool in row {
                            if widgets::icon_button(ui, tool.icon(), vec2(32.0, 30.0), self.tool == tool, true, tool.label()).clicked() {
                                self.set_tool(tool);
                            }
                        }
                    });
                }
            });
        });
    }

    fn brushes_group(&mut self, ui: &mut Ui) {
        group(ui, "Brushes", |ui| {
            let open = Popup::is_id_open(ui.ctx(), ui.next_auto_id().with("popup"));
            let response = widgets::dropdown_button(ui, Icon::Brush, vec2(48.0, ROW_H), self.tool == Tool::Brush, open, "Brushes");
            if response.clicked() {
                self.set_tool(Tool::Brush);
            }
            let frame = widgets::popup_frame(ui);
            let p = Palette::of(ui.ctx());
            Popup::menu(&response).frame(frame).show(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
                for kind in BrushKind::ALL {
                    let (rect, r) = ui.allocate_exact_size(vec2(170.0, 40.0), Sense::click());
                    let selected = self.brush == kind && self.tool == Tool::Brush;
                    let fill = if selected { p.selected } else if r.hovered() { p.hover } else { Color32::TRANSPARENT };
                    ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
                    if let Some(tex) = self.previews.brush(kind) {
                        let img = Rect::from_center_size(rect.center(), vec2(150.0, 35.0));
                        ui.painter().image(tex.id(), img, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), p.icon);
                    }
                    let label = kind.label().to_owned();
                    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label.clone()));
                    let r = r.on_hover_text(kind.label());
                    if r.clicked() {
                        self.brush = kind;
                        self.set_tool(Tool::Brush);
                    }
                }
            });
        });
    }

    fn shapes_group(&mut self, ui: &mut Ui) {
        group(ui, "Shapes", |ui| {
            let p = Palette::of(ui.ctx());
            // Gallery: 8 columns × 3 rows.
            let cell = vec2(25.0, 21.0);
            let (rect, _) = ui.allocate_exact_size(vec2(cell.x * 8.0 + 4.0, ROW_H), Sense::hover());
            ui.painter().rect(rect, CornerRadius::same(4), p.input_bg.gamma_multiply(if p.dark { 1.0 } else { 0.6 }), Stroke::new(1.0, p.border), StrokeKind::Inside);
            for (i, kind) in ShapeKind::ALL.into_iter().enumerate() {
                let (col, row) = ((i % 8) as f32, (i / 8) as f32);
                let cell_rect = Rect::from_min_size(rect.min + vec2(2.0 + col * cell.x, 1.5 + row * cell.y), cell);
                let id = ui.scope_id().with(("shape", i));
                let r = ui.interact(cell_rect, id, Sense::click());
                let selected = self.tool == Tool::Shape && self.shape_kind == kind;
                let fill = if selected { p.selected } else if r.hovered() { p.hover } else { Color32::TRANSPARENT };
                ui.painter().rect_filled(cell_rect.shrink(1.0), CornerRadius::same(3), fill);
                paint_shape_icon(ui, kind, cell_rect.shrink(2.0), p.icon);
                let label = kind.label().to_owned();
                r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label.clone()));
                if r.on_hover_text(kind.label()).clicked() {
                    self.commit_shape();
                    self.shape_kind = kind;
                    self.set_tool(Tool::Shape);
                }
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 4.0);
                ui.add_space(4.0);
                let enabled = self.tool == Tool::Shape;
                let r = widgets::labeled_dropdown(ui, Icon::Outline, "Outline", 96.0, enabled);
                self.style_popup(ui, &r, true);
                let r = widgets::labeled_dropdown(ui, Icon::ShapeFill, "Fill", 96.0, enabled);
                self.style_popup(ui, &r, false);
            });
        });
    }

    fn style_popup(&mut self, ui: &Ui, response: &egui::Response, outline: bool) {
        let frame = widgets::popup_frame(ui);
        let p = Palette::of(ui.ctx());
        Popup::menu(response).frame(frame).show(|ui| {
            ui.set_min_width(220.0);
            ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
            for style in PaintStyle::ALL {
                let current = if outline { self.outline } else { self.fill };
                let label = if outline { style.outline_label() } else { style.fill_label() };
                let (rect, r) = ui.allocate_exact_size(vec2(220.0, 34.0), Sense::click());
                let fill = if current == style { p.selected } else if r.hovered() { p.hover } else { Color32::TRANSPARENT };
                ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
                match self.previews.style(style) {
                    Some(tex) => {
                        let img = Rect::from_min_size(rect.min + vec2(8.0, 5.0), vec2(96.0, 24.0));
                        ui.painter().image(tex.id(), img, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), p.icon);
                    }
                    None if style == PaintStyle::Solid => {
                        ui.painter().hline((rect.min.x + 12.0)..=(rect.min.x + 100.0), rect.center().y, Stroke::new(4.0, p.icon));
                    }
                    None => {}
                }
                ui.painter().text(pos2(rect.min.x + 114.0, rect.center().y), Align2::LEFT_CENTER, label, theme::body(), p.text);
                let l = label.to_owned();
                r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, l.clone()));
                if r.clicked() {
                    if outline {
                        self.outline = style;
                    } else {
                        self.fill = style;
                    }
                    ui.close();
                }
            }
        });
    }

    fn size_group(&mut self, ui: &mut Ui) {
        group(ui, "Size", |ui| {
            let kind = self.tool.size_kind();
            let open = Popup::is_id_open(ui.ctx(), ui.next_auto_id().with("popup"));
            let response = widgets::dropdown_button(ui, Icon::Size, vec2(48.0, ROW_H), false, open, "Size");
            let Some(kind) = kind else {
                return;
            };
            let frame = widgets::popup_frame(ui);
            let p = Palette::of(ui.ctx());
            Popup::menu(&response)
                .frame(frame)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.set_min_width(220.0);
                    ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
                    let mut value = self.sizes.get(kind);
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        let slider = egui::Slider::new(&mut value, 1..=kind.max()).suffix(" px").show_value(true);
                        if ui.add(slider).changed() {
                            self.sizes.set(kind, value);
                        }
                    });
                    ui.add_space(4.0);
                    for &preset in kind.presets() {
                        let (rect, r) = ui.allocate_exact_size(vec2(220.0, 30.0), Sense::click());
                        let selected = self.sizes.get(kind) == preset;
                        let fill = if selected { p.selected } else if r.hovered() { p.hover } else { Color32::TRANSPARENT };
                        ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
                        let thickness = (preset as f32).min(22.0);
                        ui.painter().rect_filled(
                            Rect::from_center_size(pos2(rect.min.x + 80.0, rect.center().y), vec2(130.0, thickness)),
                            CornerRadius::same(if kind == SizeKind::Pencil || kind == SizeKind::Eraser { 0 } else { (thickness / 2.0) as u8 }),
                            p.icon,
                        );
                        ui.painter().text(pos2(rect.max.x - 10.0, rect.center().y), Align2::RIGHT_CENTER, format!("{preset} px"), theme::caption(), p.text_secondary);
                        let label = format!("{preset} px");
                        r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label.clone()));
                        if r.clicked() {
                            self.sizes.set(kind, preset);
                            ui.close();
                        }
                    }
                });
        });
    }

    fn colors_group(&mut self, ui: &mut Ui) {
        group(ui, "Colors", |ui| {
            let p = Palette::of(ui.ctx());
            // Color 1 and Color 2.
            for (slot, color, label) in [(ColorSlot::Primary, self.color1, "Color 1"), (ColorSlot::Secondary, self.color2, "Color 2")] {
                let (rect, r) = ui.allocate_exact_size(vec2(46.0, ROW_H), Sense::click());
                let active = self.active_slot == slot;
                if active {
                    ui.painter().rect_filled(rect, CornerRadius::same(4), p.selected);
                } else if r.hovered() {
                    ui.painter().rect_filled(rect, CornerRadius::same(4), p.hover);
                }
                let center = pos2(rect.center().x, rect.min.y + 22.0);
                let radius = 14.0;
                ui.painter().circle_filled(center, radius, to_color32(color));
                ui.painter().circle_stroke(center, radius, Stroke::new(1.0, p.border.gamma_multiply(1.5)));
                if active {
                    ui.painter().circle_stroke(center, radius + 3.0, Stroke::new(2.0, p.accent));
                }
                ui.painter().text(pos2(rect.center().x, rect.max.y - 12.0), Align2::CENTER_CENTER, label, egui::FontId::proportional(11.0), p.text);
                let l = label.to_owned();
                r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, l.clone()));
                if r.on_hover_text(format!("{label} ({})", color.to_hex())).clicked() {
                    self.active_slot = slot;
                }
            }
            ui.add_space(4.0);
            // Palette: two rows of defaults plus a row of custom colours.
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                ui.add_space(0.0);
                for row in 0..3 {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                        for col in 0..10 {
                            let color = if row < 2 { Some(DEFAULT_PALETTE[row * 10 + col]) } else { self.custom_colors.get(col).copied() };
                            let label = match color {
                                Some(c) => c.to_hex(),
                                None => "Custom color".to_owned(),
                            };
                            let r = widgets::color_circle(ui, color, 18.0, &label);
                            if let Some(c) = color {
                                if r.clicked() {
                                    let slot = self.active_slot;
                                    self.set_color(slot, c);
                                }
                                if r.secondary_clicked() {
                                    self.set_color(ColorSlot::Secondary, c);
                                }
                            } else if r.clicked() {
                                self.run(ui, Command::EditColors);
                            }
                        }
                    });
                }
            });
            ui.add_space(4.0);
            let (rect, r) = ui.allocate_exact_size(vec2(44.0, ROW_H), Sense::click());
            if r.hovered() {
                ui.painter().rect_filled(rect, CornerRadius::same(4), p.hover);
            }
            widgets::paint_hue_wheel(ui.painter(), pos2(rect.center().x, rect.min.y + 22.0), 13.0);
            ui.painter().text(pos2(rect.center().x, rect.max.y - 13.0), Align2::CENTER_CENTER, "Edit\ncolors", egui::FontId::proportional(10.5), p.text);
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Edit colors"));
            if r.on_hover_text("Edit colors").clicked() {
                self.run(ui, Command::EditColors);
            }
        });
    }

    fn layers_group(&mut self, ui: &mut Ui) {
        group(ui, "Layers", |ui| {
            let (rect, r) = ui.allocate_exact_size(vec2(52.0, ROW_H), Sense::click());
            let p = Palette::of(ui.ctx());
            let fill = if self.view.show_layers { p.selected } else if r.hovered() { p.hover } else { Color32::TRANSPARENT };
            ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
            Icon::Layers.paint(ui, Rect::from_center_size(pos2(rect.center().x, rect.min.y + 24.0), Vec2::splat(28.0)), 26.0, p.icon);
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Layers"));
            if r.on_hover_text("Layers").clicked() {
                self.run(ui, Command::ToggleLayers);
            }
        });
    }
}
