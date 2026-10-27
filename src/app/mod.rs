// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! The OpenBrush application: state, frame loop and window chrome.

mod canvas;
mod commands;
mod dialogs;
mod edit;
mod files;
mod layers;
mod menus;
mod render;
mod settings;
pub mod state;
mod statusbar;
mod textbar;
mod toolbar;
mod tools;

use std::path::PathBuf;
use std::time::SystemTime;

use egui::{Frame, Margin};
use tiny_skia::{IntRect, Pixmap, PixmapPaint};

pub use commands::Command;
pub use state::{ColorSlot, SelectMode, Sizes, Tool, View};

use crate::engine::brush::BrushKind;
use crate::engine::color::Rgba;
use crate::engine::document::{self, Document};
use crate::engine::history::History;
use crate::engine::io::FileFormat;
use crate::engine::raster;
use crate::engine::shapes::{PaintStyle, ShapeKind};
use crate::engine::text::{TextAlign, TextEngine, TextStyle};
use crate::ui::theme::{self, Palette, ThemePref};
use dialogs::Dialog;
use render::CanvasTextures;
use tools::{ActiveSelection, Gesture, ShapeEdit, TextBox};

pub const APP_NAME: &str = "OpenBrush";

/// What we know about the file being edited.
#[derive(Clone, Debug, Default)]
pub struct FileInfo {
    pub path: Option<PathBuf>,
    pub format: Option<FileFormat>,
    /// `History::state_id` at the last save.
    pub saved_state: u64,
    pub size_on_disk: Option<u64>,
    pub modified: Option<SystemTime>,
}

impl FileInfo {
    pub fn display_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_owned())
    }
}

/// Loads system fonts on a background thread; the Text tool waits for it only if needed.
pub(crate) struct Fonts {
    engine: Option<TextEngine>,
    loading: Option<std::thread::JoinHandle<TextEngine>>,
}

impl Fonts {
    fn load_in_background() -> Self {
        Self {
            engine: None,
            loading: Some(std::thread::spawn(TextEngine::new)),
        }
    }

    fn bundled() -> Self {
        Self {
            engine: Some(TextEngine::bundled_only()),
            loading: None,
        }
    }

    pub(crate) fn get(&mut self) -> &mut TextEngine {
        if self.engine.is_none() {
            let engine = self
                .loading
                .take()
                .and_then(|h| h.join().ok())
                .unwrap_or_else(TextEngine::bundled_only);
            self.engine = Some(engine);
        }
        self.engine.as_mut().expect("engine was just set")
    }
}

/// Options for creating the app; tests use lighter-weight settings.
#[derive(Clone, Copy, Debug)]
pub struct AppOptions {
    pub load_system_fonts: bool,
}

impl Default for AppOptions {
    fn default() -> Self {
        Self { load_system_fonts: true }
    }
}

pub struct PaintApp {
    pub doc: Document,
    pub history: History,
    pub file: FileInfo,

    pub tool: Tool,
    pub previous_tool: Tool,
    pub select_mode: SelectMode,
    pub transparent_selection: bool,
    pub brush: BrushKind,
    pub shape_kind: ShapeKind,
    pub outline: PaintStyle,
    pub fill: PaintStyle,
    pub sizes: Sizes,
    pub color1: Rgba,
    pub color2: Rgba,
    pub active_slot: ColorSlot,
    pub custom_colors: Vec<Rgba>,
    pub text_style: TextStyle,
    pub text_background: bool,

    pub view: View,
    pub theme: ThemePref,
    pub recent: Vec<PathBuf>,

    pub(crate) gesture: Gesture,
    pub(crate) selection: Option<ActiveSelection>,
    pub(crate) shape: Option<ShapeEdit>,
    pub(crate) text: Option<TextBox>,
    /// Document-sized scratch image for the stroke, shape or text being edited.
    pub(crate) draft: Option<Pixmap>,
    pub(crate) draft_paint: PixmapPaint,
    pub(crate) draft_area: Option<IntRect>,

    pub(crate) canvas: CanvasTextures,
    pub(crate) previews: toolbar::Previews,
    pub(crate) thumbnails: layers::Thumbnails,
    pub(crate) opacity_before: Option<Document>,
    /// The canvas area of the window, from the last frame.
    pub(crate) viewport: egui::Rect,
    pub(crate) ctx: Option<egui::Context>,
    pub(crate) dialog: Option<Dialog>,
    pub(crate) fonts: Fonts,
    pub(crate) clipboard: Option<arboard::Clipboard>,
    pub(crate) cursor_pixel: Option<(i32, i32)>,
    pub(crate) applied_dark: Option<bool>,
    pub(crate) close_confirmed: bool,
    pub(crate) last_title: String,
    pub(crate) seed: u64,
}

impl PaintApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::with_options(cc, AppOptions::default())
    }

    pub fn with_options(cc: &eframe::CreationContext<'_>, options: AppOptions) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        theme::install_fonts(&cc.egui_ctx);
        let mut app = Self::headless(options);
        if let Some(storage) = cc.storage {
            app.load_settings(storage);
        }
        app
    }

    /// An app with default settings that hasn't been attached to a window yet.
    pub fn headless(options: AppOptions) -> Self {
        let fonts = if options.load_system_fonts { Fonts::load_in_background() } else { Fonts::bundled() };
        Self {
            doc: Document::new(document::DEFAULT_WIDTH, document::DEFAULT_HEIGHT, Rgba::WHITE),
            history: History::new(),
            file: FileInfo::default(),
            tool: Tool::Pencil,
            previous_tool: Tool::Pencil,
            select_mode: SelectMode::Rectangle,
            transparent_selection: false,
            brush: state::default_brush(),
            shape_kind: ShapeKind::Rectangle,
            outline: PaintStyle::Solid,
            fill: PaintStyle::None,
            sizes: Sizes::default(),
            color1: Rgba::BLACK,
            color2: Rgba::WHITE,
            active_slot: ColorSlot::Primary,
            custom_colors: Vec::new(),
            text_style: TextStyle {
                family: String::new(),
                size_pt: 11.0,
                bold: false,
                italic: false,
                underline: false,
                strikethrough: false,
                align: TextAlign::Left,
                color: Rgba::BLACK,
                background: None,
            },
            text_background: false,
            view: View::default(),
            theme: ThemePref::System,
            recent: Vec::new(),
            gesture: Gesture::None,
            selection: None,
            shape: None,
            text: None,
            draft: None,
            draft_paint: PixmapPaint::default(),
            draft_area: None,
            canvas: CanvasTextures::default(),
            previews: Default::default(),
            thumbnails: Default::default(),
            opacity_before: None,
            viewport: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 720.0)),
            ctx: None,
            dialog: None,
            fonts,
            clipboard: None,
            cursor_pixel: None,
            applied_dark: None,
            close_confirmed: false,
            last_title: String::new(),
            seed: 1,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.history.state_id() != self.file.saved_state
    }

    pub fn title(&self) -> String {
        let dirty = if self.is_dirty() { "*" } else { "" };
        format!("{dirty}{} - {APP_NAME}", self.file.display_name())
    }

    /// Marks part of the image as needing to be redrawn.
    pub(crate) fn invalidate(&mut self, r: IntRect) {
        self.canvas.invalidate(r);
    }

    pub(crate) fn invalidate_all(&mut self) {
        self.canvas.invalidate_all();
    }

    pub(crate) fn next_seed(&mut self) -> u64 {
        self.seed = self.seed.wrapping_add(1);
        self.seed
    }

    /// The image as shown on screen for `region`, including any live preview.
    pub(crate) fn compose(&self, region: IntRect) -> Pixmap {
        let mut out = raster::new_pixmap(region.width(), region.height());
        let preview = self.active_preview(region);
        self.doc.composite_into(region, &mut out, preview.as_ref());
        out
    }

    /// The active layer with in-progress edits drawn onto it, or `None` if nothing is live.
    fn active_preview(&self, region: IntRect) -> Option<Pixmap> {
        let floating = self.selection.as_ref().filter(|s| s.sel.is_floating());
        if self.draft.is_none() && floating.is_none() {
            return None;
        }
        let mut px = raster::new_pixmap(region.width(), region.height());
        let layer = &self.doc.active_layer().pixels;
        if let Some(r) = raster::clamp_to(region, layer.width(), layer.height()) {
            raster::replace_region(&mut px, &raster::copy_region(layer, r), r.x() - region.x(), r.y() - region.y());
        }
        if let Some(draft) = &self.draft {
            px.draw_pixmap(
                -region.x(),
                -region.y(),
                draft.as_ref(),
                &self.draft_paint,
                tiny_skia::Transform::identity(),
                None,
            );
        }
        if let Some(sel) = floating {
            let key = self.transparent_selection.then_some(self.color2);
            sel.sel.draw_floating(&mut px, (region.x(), region.y()), key);
        }
        Some(px)
    }

    fn apply_theme(&mut self, ctx: &egui::Context) {
        let dark = match self.theme {
            ThemePref::Light => false,
            ThemePref::Dark => true,
            ThemePref::System => ctx.system_theme().is_some_and(|t| t == egui::Theme::Dark),
        };
        if self.applied_dark != Some(dark) {
            theme::apply(ctx, dark);
            self.applied_dark = Some(dark);
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = self.title();
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }

    fn chrome_frame(p: &Palette) -> Frame {
        Frame::new().fill(p.chrome).inner_margin(Margin::symmetric(8, 2))
    }
}

impl eframe::App for PaintApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.apply_theme(&ctx);
        self.handle_close_request(&ctx);
        self.handle_dropped_files(&ctx);
        self.handle_shortcuts(&ctx);
        self.update_title(&ctx);
        let p = Palette::of(&ctx);

        if !self.view.fullscreen {
            egui::Panel::top("menu_bar")
                .frame(Self::chrome_frame(&p).inner_margin(Margin { left: 6, right: 8, top: 4, bottom: 2 }))
                .show(ui, |ui| self.menu_bar(ui));
            egui::Panel::top("toolbar")
                .frame(Self::chrome_frame(&p).inner_margin(Margin { left: 6, right: 8, top: 2, bottom: 4 }))
                .show(ui, |ui| self.toolbar(ui));
            if self.tool == Tool::Text && self.text.is_some() {
                egui::Panel::top("text_bar")
                    .frame(Self::chrome_frame(&p).inner_margin(Margin::symmetric(10, 4)))
                    .show(ui, |ui| self.text_bar(ui));
            }
            if self.view.show_status_bar {
                egui::Panel::bottom("status_bar")
                    .frame(Self::chrome_frame(&p).inner_margin(Margin::symmetric(12, 2)))
                    .show(ui, |ui| self.status_bar(ui));
            }
            if self.view.show_layers {
                egui::Panel::right("layers")
                    .frame(Frame::new().fill(p.chrome).inner_margin(Margin::same(10)))
                    .resizable(false)
                    .exact_size(250.0)
                    .show(ui, |ui| self.layers_panel(ui));
            }
        }
        egui::CentralPanel::no_frame().show(ui, |ui| self.workspace(ui));
        self.thumbnail_window(&ctx);
        self.show_dialogs(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.save_settings(storage);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let p = Palette::for_dark(self.applied_dark.unwrap_or(false));
        p.workspace.to_normalized_gamma_f32()
    }
}
