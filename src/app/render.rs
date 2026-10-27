// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Uploads the composited image to the GPU as a grid of texture tiles.
//!
//! Tiling keeps every texture within GPU size limits and lets small edits re-upload
//! only the tiles they touch.

use egui::{Color32, ColorImage, Painter, Pos2, Rect, TextureFilter, TextureHandle, TextureOptions, TextureWrapMode, pos2};
use tiny_skia::{IntRect, Pixmap};

use crate::engine::raster;

const TILE: u32 = 1024;

struct Tile {
    rect: IntRect,
    texture: TextureHandle,
}

#[derive(Default)]
pub struct CanvasTextures {
    tiles: Vec<Tile>,
    size: (u32, u32),
    dirty: Option<IntRect>,
    all_dirty: bool,
}

fn options(smooth: bool) -> TextureOptions {
    TextureOptions {
        magnification: if smooth { TextureFilter::Linear } else { TextureFilter::Nearest },
        minification: TextureFilter::Linear,
        wrap_mode: TextureWrapMode::ClampToEdge,
        mipmap_mode: None,
    }
}

fn to_color_image(p: &Pixmap) -> ColorImage {
    ColorImage::from_rgba_premultiplied([p.width() as usize, p.height() as usize], p.data())
}

impl CanvasTextures {
    pub fn invalidate(&mut self, r: IntRect) {
        self.dirty = raster::union_opt(self.dirty, Some(r));
    }

    pub fn invalidate_all(&mut self) {
        self.all_dirty = true;
    }

    /// Brings the textures up to date. `compose` must return the composited image for a region.
    pub fn sync(&mut self, ctx: &egui::Context, width: u32, height: u32, mut compose: impl FnMut(IntRect) -> Pixmap) {
        let opts = options(false);
        if self.size != (width, height) || self.tiles.is_empty() {
            self.size = (width, height);
            self.tiles.clear();
            for ty in (0..height).step_by(TILE as usize) {
                for tx in (0..width).step_by(TILE as usize) {
                    let rect = IntRect::from_xywh(tx as i32, ty as i32, TILE.min(width - tx), TILE.min(height - ty))
                        .expect("tile is non-empty");
                    let blank = ColorImage::filled([rect.width() as usize, rect.height() as usize], Color32::TRANSPARENT);
                    let texture = ctx.load_texture(format!("canvas-{tx}-{ty}"), blank, opts);
                    self.tiles.push(Tile { rect, texture });
                }
            }
            self.all_dirty = true;
        }
        let region = if std::mem::take(&mut self.all_dirty) {
            self.dirty = None;
            IntRect::from_xywh(0, 0, width, height)
        } else {
            self.dirty.take().and_then(|r| raster::clamp_to(r, width, height))
        };
        let Some(region) = region else {
            return;
        };
        for tile in &mut self.tiles {
            let Some(part) = raster::intersect(region, tile.rect) else {
                continue;
            };
            let pixels = compose(part);
            let offset = [(part.x() - tile.rect.x()) as usize, (part.y() - tile.rect.y()) as usize];
            if part == tile.rect {
                tile.texture.set(to_color_image(&pixels), opts);
            } else {
                tile.texture.set_partial(offset, to_color_image(&pixels), opts);
            }
        }
    }

    /// Draws the tiles with the image's top-left at `origin`, `zoom` screen points per pixel.
    pub fn paint(&self, painter: &Painter, origin: Pos2, zoom: f32) {
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        for tile in &self.tiles {
            let min = origin + egui::vec2(tile.rect.x() as f32, tile.rect.y() as f32) * zoom;
            let max = origin + egui::vec2(tile.rect.right() as f32, tile.rect.bottom() as f32) * zoom;
            painter.image(tile.texture.id(), Rect::from_min_max(min, max), uv, Color32::WHITE);
        }
    }
}
