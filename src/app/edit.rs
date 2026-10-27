// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Edit and Image operations: undo, clipboard, selection, transforms and layers.

use std::borrow::Cow;

use tiny_skia::{IntRect, Pixmap};

use super::dialogs::Dialog;
use super::state::{SelectMode, Tool};
use super::tools::ActiveSelection;
use super::PaintApp;
use crate::engine::color::Rgba;
use crate::engine::document::BlendMode;
use crate::engine::raster;
use crate::engine::selection::Selection;
use crate::engine::transform;

impl PaintApp {
    /// Called after anything that may have changed the document's size or layers.
    pub(crate) fn document_changed(&mut self) {
        self.draft = None;
        self.draft_area = None;
        self.doc.active = self.doc.active.min(self.doc.layers.len() - 1);
        self.invalidate_all();
    }

    // ------------------------------------------------------------------ undo

    pub fn undo(&mut self) {
        // An edit in progress is undone first, before touching history.
        if self.selection.as_ref().is_some_and(|s| s.sel.is_floating()) {
            self.cancel_selection();
            return;
        }
        if self.shape.is_some() {
            self.cancel_shape();
            return;
        }
        if self.text.is_some() {
            self.text = None;
            self.discard_draft();
            return;
        }
        self.selection = None;
        if self.history.undo(&mut self.doc).is_some() {
            self.document_changed();
        }
    }

    pub fn redo(&mut self) {
        self.commit_all();
        if self.history.redo(&mut self.doc).is_some() {
            self.document_changed();
        }
    }

    // ------------------------------------------------------------------ clipboard

    fn clipboard(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        self.clipboard.as_mut()
    }

    /// The selected pixels, or `None` without a selection.
    pub fn selected_pixels(&self) -> Option<Pixmap> {
        let active = self.selection.as_ref()?;
        let index = self.doc.layer_index(active.layer)?;
        active.sel.extract(&self.doc.layers[index].pixels)
    }

    pub fn copy(&mut self) {
        let Some(pixels) = self.selected_pixels() else {
            return;
        };
        let (w, h) = (pixels.width() as usize, pixels.height() as usize);
        let bytes = raster::to_rgba8(&pixels);
        if let Some(cb) = self.clipboard() {
            let _ = cb.set_image(arboard::ImageData {
                width: w,
                height: h,
                bytes: Cow::Owned(bytes),
            });
        }
    }

    pub fn cut(&mut self) {
        self.copy();
        self.delete_selection();
    }

    fn clipboard_image(&mut self) -> Option<Pixmap> {
        let cb = self.clipboard()?;
        if let Ok(img) = cb.get_image() {
            return raster::from_rgba8(img.width as u32, img.height as u32, &img.bytes);
        }
        // A copied file path also works, as in Paint.
        let text = cb.get_text().ok()?;
        let path = std::path::Path::new(text.trim().trim_matches('"'));
        if path.is_file() { crate::engine::io::load(path).ok() } else { None }
    }

    pub fn paste(&mut self) {
        if let Some(image) = self.clipboard_image() {
            self.paste_image(image);
        }
    }

    /// Pastes an image as a floating selection. If it's larger than the canvas, asks first.
    pub fn paste_image(&mut self, image: Pixmap) {
        self.commit_all();
        if image.width() > self.doc.width() || image.height() > self.doc.height() {
            self.dialog = Some(Dialog::PasteLarger { image });
            return;
        }
        self.place_pasted(image);
    }

    /// Places an image at the top-left of the visible part of the canvas.
    pub(crate) fn place_pasted(&mut self, image: Pixmap) {
        let origin = self.canvas_origin(self.viewport);
        let top_left = self.to_doc(self.viewport.min, origin);
        let max_x = self.doc.width().saturating_sub(image.width()) as f32;
        let max_y = self.doc.height().saturating_sub(image.height()) as f32;
        let x = top_left.x.max(0.0).min(max_x).floor() as i32;
        let y = top_left.y.max(0.0).min(max_y).floor() as i32;
        self.tool = Tool::Select;
        self.select_mode = SelectMode::Rectangle;
        let layer = self.doc.active_layer();
        let mut sel = Selection::pasted(image, x, y);
        // A pasted selection has no source area to clear.
        sel.origin = IntRect::from_xywh(x, y, 1, 1).expect("valid");
        let area = sel.floating_area();
        self.selection = Some(ActiveSelection {
            sel,
            layer: layer.id,
            before: Some(layer.pixels.clone()),
        });
        if let Some(a) = area {
            self.invalidate(a);
        }
    }

    /// Grows the canvas to fit `image`, then pastes it.
    pub(crate) fn enlarge_and_paste(&mut self, image: Pixmap) {
        let w = self.doc.width().max(image.width());
        let h = self.doc.height().max(image.height());
        self.resize_canvas(w, h);
        self.place_pasted(image);
    }

    // ------------------------------------------------------------------ selection

    pub fn select_all(&mut self) {
        self.commit_all();
        self.tool = Tool::Select;
        self.selection = Some(ActiveSelection {
            sel: Selection::all(self.doc.width(), self.doc.height()),
            layer: self.doc.active_layer().id,
            before: None,
        });
    }

    pub fn invert_selection(&mut self) {
        let Some(active) = self.selection.as_ref() else {
            return;
        };
        let inverted = active.sel.inverted(self.doc.width(), self.doc.height());
        let layer = active.layer;
        self.selection = inverted.map(|sel| ActiveSelection { sel, layer, before: None });
    }

    /// Clears the selected pixels (or discards a floating selection).
    pub fn delete_selection(&mut self) {
        if self.shape.is_some() {
            self.cancel_shape();
            return;
        }
        let Some(active) = self.selection.take() else {
            return;
        };
        let Some(index) = self.doc.layer_index(active.layer) else {
            return;
        };
        let layer = &mut self.doc.layers[index];
        let id = layer.id;
        match active.before {
            Some(before) => {
                // Floating: the source was already cleared when lifted; just drop the pixels.
                let area = raster::union_opt(Some(active.sel.origin), active.sel.floating_area())
                    .and_then(|r| raster::clamp_to(r, layer.pixels.width(), layer.pixels.height()));
                if let Some(area) = area {
                    self.history.push_pixels("Delete", id, area, raster::copy_region(&before, area));
                    self.invalidate(area);
                }
            }
            None => {
                let Some(area) = raster::clamp_to(active.sel.origin, layer.pixels.width(), layer.pixels.height()) else {
                    return;
                };
                let before = raster::copy_region(&layer.pixels, area);
                active.sel.clear(&mut layer.pixels);
                self.history.push_pixels("Delete", id, area, before);
                self.invalidate(area);
            }
        }
    }

    // ------------------------------------------------------------------ image

    /// Crops the image to the selection.
    pub fn crop(&mut self) {
        let Some(bounds) = self.selection.as_ref().map(|s| s.sel.bounds()) else {
            return;
        };
        self.commit_all();
        let Some(r) = IntRect::from_ltrb(
            bounds.left().round() as i32,
            bounds.top().round() as i32,
            bounds.right().round() as i32,
            bounds.bottom().round() as i32,
        )
        .and_then(|r| raster::clamp_to(r, self.doc.width(), self.doc.height())) else {
            return;
        };
        let before = self.doc.clone();
        self.doc.map_layers(|p| transform::crop(p, r).unwrap_or_else(|| p.clone()));
        self.history.push_document("Crop", before);
        self.document_changed();
        self.view.fit_pending = self.doc.width() as f32 * self.view.zoom > self.viewport.width();
    }

    /// Rotates or flips the selection if there is one, otherwise the whole picture.
    pub fn transform_image(&mut self, label: &str, f: fn(&Pixmap) -> Pixmap) {
        if self.selection.is_some() {
            self.lift_selection(false);
            if let Some(active) = self.selection.as_mut() {
                let old = active.sel.floating_area();
                if let Some(pixels) = active.sel.floating.as_ref().map(|fl| {
                    // Bake any pending scale into the pixels before transforming.
                    let (w, h) = (fl.rect.width().round().max(1.0) as u32, fl.rect.height().round().max(1.0) as u32);
                    transform::scale(&fl.pixels, w, h, true)
                }) {
                    active.sel.replace_floating(f(&pixels));
                }
                let new = active.sel.floating_area();
                if let Some(r) = raster::union_opt(old, new) {
                    self.invalidate(r);
                }
            }
            return;
        }
        self.commit_all();
        let before = self.doc.clone();
        self.doc.map_layers(f);
        self.history.push_document(label, before);
        self.document_changed();
    }

    /// Scales and skews the selection, or the whole picture.
    pub fn resize_and_skew(&mut self, width: u32, height: u32, skew_h: f32, skew_v: f32) {
        let apply = |p: &Pixmap| {
            let scaled = transform::scale(p, width, height, true);
            if skew_h != 0.0 || skew_v != 0.0 { transform::skew(&scaled, skew_h, skew_v) } else { scaled }
        };
        if self.selection.is_some() {
            self.lift_selection(false);
            if let Some(active) = self.selection.as_mut() {
                let old = active.sel.floating_area();
                if let Some(pixels) = active.sel.floating.as_ref().map(|fl| fl.pixels.clone()) {
                    active.sel.replace_floating(apply(&pixels));
                }
                if let Some(r) = raster::union_opt(old, active.sel.floating_area()) {
                    self.invalidate(r);
                }
            }
            return;
        }
        self.commit_all();
        let before = self.doc.clone();
        let bottom = self.doc.layers.first().map(|l| l.id);
        let mut first = true;
        self.doc.map_layers(|p| {
            let mut out = apply(p);
            // Skewing exposes new area; on the bottom layer it's white, like the canvas.
            if first && (skew_h != 0.0 || skew_v != 0.0) {
                out = raster::flatten_onto(&out, Rgba::WHITE);
            }
            first = false;
            out
        });
        let _ = bottom;
        self.history.push_document("Resize", before);
        self.document_changed();
        self.view.fit_pending = true;
    }

    /// Changes the canvas size without scaling the picture.
    pub fn resize_canvas(&mut self, width: u32, height: u32) {
        self.commit_all();
        let before = self.doc.clone();
        self.doc.resize_canvas(width, height, 0, 0, Rgba::WHITE);
        self.history.push_document("Resize canvas", before);
        self.document_changed();
    }

    // ------------------------------------------------------------------ layers

    fn layer_edit(&mut self, label: &str, f: impl FnOnce(&mut crate::engine::document::Document) -> bool) {
        self.commit_all();
        let before = self.doc.clone();
        if f(&mut self.doc) {
            self.history.push_document(label, before);
            self.document_changed();
        }
    }

    pub fn add_layer(&mut self) {
        self.layer_edit("Add layer", |d| {
            d.add_layer();
            true
        });
        self.view.show_layers = true;
    }

    pub fn duplicate_layer(&mut self, index: usize) {
        self.layer_edit("Duplicate layer", |d| {
            d.duplicate_layer(index);
            true
        });
    }

    pub fn delete_layer(&mut self, index: usize) {
        self.layer_edit("Delete layer", |d| d.delete_layer(index));
    }

    pub fn merge_down(&mut self, index: usize) {
        self.layer_edit("Merge layer down", |d| d.merge_down(index));
    }

    pub fn move_layer(&mut self, from: usize, to: usize) {
        self.layer_edit("Move layer", |d| d.move_layer(from, to));
    }

    pub fn set_layer_visible(&mut self, index: usize, visible: bool) {
        self.layer_edit(if visible { "Show layer" } else { "Hide layer" }, |d| {
            let changed = d.layers[index].visible != visible;
            d.layers[index].visible = visible;
            changed
        });
    }

    pub fn set_layer_blend(&mut self, index: usize, blend: BlendMode) {
        self.layer_edit("Blend mode", |d| {
            let changed = d.layers[index].blend != blend;
            d.layers[index].blend = blend;
            changed
        });
    }

    /// Sets a layer's opacity without recording history (the caller records slider drags once).
    pub fn set_layer_opacity(&mut self, index: usize, opacity: f32) {
        self.doc.layers[index].opacity = opacity.clamp(0.0, 1.0);
        self.invalidate_all();
    }

    pub fn select_layer(&mut self, index: usize) {
        if index < self.doc.layers.len() && index != self.doc.active {
            self.commit_all();
            self.doc.active = index;
        }
    }
}
