// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! The layered image being edited.

use tiny_skia::{FilterQuality, IntRect, Pixmap, PixmapPaint, Transform};

use super::color::Rgba;
use super::raster;

/// How a layer is combined with the layers beneath it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl BlendMode {
    pub const ALL: [BlendMode; 16] = [
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::ColorDodge,
        BlendMode::ColorBurn,
        BlendMode::HardLight,
        BlendMode::SoftLight,
        BlendMode::Difference,
        BlendMode::Exclusion,
        BlendMode::Hue,
        BlendMode::Saturation,
        BlendMode::Color,
        BlendMode::Luminosity,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BlendMode::Normal => "Normal",
            BlendMode::Multiply => "Multiply",
            BlendMode::Screen => "Screen",
            BlendMode::Overlay => "Overlay",
            BlendMode::Darken => "Darken",
            BlendMode::Lighten => "Lighten",
            BlendMode::ColorDodge => "Color dodge",
            BlendMode::ColorBurn => "Color burn",
            BlendMode::HardLight => "Hard light",
            BlendMode::SoftLight => "Soft light",
            BlendMode::Difference => "Difference",
            BlendMode::Exclusion => "Exclusion",
            BlendMode::Hue => "Hue",
            BlendMode::Saturation => "Saturation",
            BlendMode::Color => "Color",
            BlendMode::Luminosity => "Luminosity",
        }
    }

    pub fn to_skia(self) -> tiny_skia::BlendMode {
        use tiny_skia::BlendMode as B;
        match self {
            BlendMode::Normal => B::SourceOver,
            BlendMode::Multiply => B::Multiply,
            BlendMode::Screen => B::Screen,
            BlendMode::Overlay => B::Overlay,
            BlendMode::Darken => B::Darken,
            BlendMode::Lighten => B::Lighten,
            BlendMode::ColorDodge => B::ColorDodge,
            BlendMode::ColorBurn => B::ColorBurn,
            BlendMode::HardLight => B::HardLight,
            BlendMode::SoftLight => B::SoftLight,
            BlendMode::Difference => B::Difference,
            BlendMode::Exclusion => B::Exclusion,
            BlendMode::Hue => B::Hue,
            BlendMode::Saturation => B::Saturation,
            BlendMode::Color => B::Color,
            BlendMode::Luminosity => B::Luminosity,
        }
    }
}

pub type LayerId = u64;

#[derive(Clone)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub pixels: Pixmap,
    pub visible: bool,
    /// 0.0 – 1.0
    pub opacity: f32,
    pub blend: BlendMode,
}

impl Layer {
    fn paint(&self) -> PixmapPaint {
        PixmapPaint {
            opacity: self.opacity.clamp(0.0, 1.0),
            blend_mode: self.blend.to_skia(),
            quality: FilterQuality::Nearest,
        }
    }
}

/// A stack of equally-sized layers, bottom first.
#[derive(Clone)]
pub struct Document {
    width: u32,
    height: u32,
    pub layers: Vec<Layer>,
    /// Index of the layer that tools draw on.
    pub active: usize,
    next_id: LayerId,
}

pub const DEFAULT_WIDTH: u32 = 1152;
pub const DEFAULT_HEIGHT: u32 = 648;
pub const MAX_DIMENSION: u32 = 16_384;

impl Document {
    /// A new single-layer document filled with `background`.
    pub fn new(width: u32, height: u32, background: Rgba) -> Self {
        Self::from_pixmap(raster::filled(width, height, background))
    }

    pub fn from_pixmap(pixels: Pixmap) -> Self {
        let mut doc = Self {
            width: pixels.width(),
            height: pixels.height(),
            layers: Vec::new(),
            active: 0,
            next_id: 1,
        };
        let layer = doc.make_layer(pixels);
        doc.layers.push(layer);
        doc
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn bounds(&self) -> IntRect {
        IntRect::from_xywh(0, 0, self.width, self.height).expect("document is non-empty")
    }

    fn make_layer(&mut self, pixels: Pixmap) -> Layer {
        let id = self.next_id;
        self.next_id += 1;
        Layer {
            id,
            name: format!("Layer {id}"),
            pixels,
            visible: true,
            opacity: 1.0,
            blend: BlendMode::Normal,
        }
    }

    pub fn active_layer(&self) -> &Layer {
        &self.layers[self.active]
    }

    pub fn active_layer_mut(&mut self) -> &mut Layer {
        &mut self.layers[self.active]
    }

    pub fn layer_index(&self, id: LayerId) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    /// Adds a transparent layer above the active one and makes it active.
    pub fn add_layer(&mut self) -> usize {
        let layer = self.make_layer(raster::new_pixmap(self.width, self.height));
        self.active = (self.active + 1).min(self.layers.len());
        self.layers.insert(self.active, layer);
        self.active
    }

    pub fn duplicate_layer(&mut self, index: usize) -> usize {
        let source = &self.layers[index];
        let (pixels, name, visible, opacity, blend) = (
            source.pixels.clone(),
            format!("{} copy", source.name),
            source.visible,
            source.opacity,
            source.blend,
        );
        let mut layer = self.make_layer(pixels);
        layer.name = name;
        layer.visible = visible;
        layer.opacity = opacity;
        layer.blend = blend;
        self.layers.insert(index + 1, layer);
        self.active = index + 1;
        self.active
    }

    /// Removes a layer. The last remaining layer can't be deleted.
    pub fn delete_layer(&mut self, index: usize) -> bool {
        if self.layers.len() <= 1 || index >= self.layers.len() {
            return false;
        }
        self.layers.remove(index);
        if self.active >= self.layers.len() || self.active > index {
            self.active = self.active.saturating_sub(1);
        }
        self.active = self.active.min(self.layers.len() - 1);
        true
    }

    /// Merges a layer into the one beneath it.
    pub fn merge_down(&mut self, index: usize) -> bool {
        if index == 0 || index >= self.layers.len() {
            return false;
        }
        let upper = self.layers.remove(index);
        if upper.visible {
            let paint = upper.paint();
            self.layers[index - 1].pixels.draw_pixmap(
                0,
                0,
                upper.pixels.as_ref(),
                &paint,
                Transform::identity(),
                None,
            );
        }
        self.active = index - 1;
        true
    }

    /// Moves a layer to a new position in the stack, keeping it active.
    pub fn move_layer(&mut self, from: usize, to: usize) -> bool {
        if from >= self.layers.len() || to >= self.layers.len() || from == to {
            return false;
        }
        let layer = self.layers.remove(from);
        self.layers.insert(to, layer);
        self.active = to;
        true
    }

    /// Composites all visible layers within `region` into `out` (which must be region-sized).
    /// When `active_override` is given it replaces the active layer's pixels for this region,
    /// which is how in-progress strokes and floating selections are previewed.
    pub fn composite_into(&self, region: IntRect, out: &mut Pixmap, active_override: Option<&Pixmap>) {
        out.fill(tiny_skia::Color::TRANSPARENT);
        for (i, layer) in self.layers.iter().enumerate() {
            if !layer.visible {
                continue;
            }
            let paint = layer.paint();
            match active_override {
                Some(over) if i == self.active => {
                    out.draw_pixmap(0, 0, over.as_ref(), &paint, Transform::identity(), None);
                }
                _ => out.draw_pixmap(
                    -region.x(),
                    -region.y(),
                    layer.pixels.as_ref(),
                    &paint,
                    Transform::identity(),
                    None,
                ),
            }
        }
    }

    /// All visible layers merged into one image.
    pub fn flatten(&self) -> Pixmap {
        let mut out = raster::new_pixmap(self.width, self.height);
        self.composite_into(self.bounds(), &mut out, None);
        out
    }

    /// Replaces the image with a single layer.
    pub fn flatten_in_place(&mut self) {
        let flat = self.flatten();
        let mut layer = self.make_layer(flat);
        layer.name = self.layers[0].name.clone();
        self.layers = vec![layer];
        self.active = 0;
    }

    /// Changes the canvas size without scaling. Content moves by `(dx, dy)`.
    /// New area on the bottom layer is filled with `background`; other layers stay transparent.
    pub fn resize_canvas(&mut self, width: u32, height: u32, dx: i32, dy: i32, background: Rgba) {
        let width = width.clamp(1, MAX_DIMENSION);
        let height = height.clamp(1, MAX_DIMENSION);
        for (i, layer) in self.layers.iter_mut().enumerate() {
            let fill = if i == 0 { background } else { Rgba::TRANSPARENT };
            let mut pixels = raster::filled(width, height, fill);
            raster::replace_region(&mut pixels, &layer.pixels, dx, dy);
            layer.pixels = pixels;
        }
        self.width = width;
        self.height = height;
    }

    /// Applies a whole-image transform (rotate, flip, scale…) to every layer.
    pub fn map_layers(&mut self, mut f: impl FnMut(&Pixmap) -> Pixmap) {
        for layer in &mut self.layers {
            layer.pixels = f(&layer.pixels);
        }
        if let Some(first) = self.layers.first() {
            self.width = first.pixels.width();
            self.height = first.pixels.height();
        }
    }

    /// Approximate memory used by the pixel data, in bytes.
    pub fn byte_size(&self) -> usize {
        self.layers.iter().map(|l| l.pixels.data().len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::raster::get_pixel;

    #[test]
    fn new_document_is_white() {
        let doc = Document::new(4, 3, Rgba::WHITE);
        assert_eq!((doc.width(), doc.height()), (4, 3));
        assert_eq!(doc.layers.len(), 1);
        assert_eq!(get_pixel(&doc.flatten(), 3, 2), Some(Rgba::WHITE));
    }

    #[test]
    fn layer_stack_operations() {
        let mut doc = Document::new(4, 4, Rgba::WHITE);
        let top = doc.add_layer();
        assert_eq!(top, 1);
        assert_eq!(doc.active, 1);
        raster::fill_region(&mut doc.layers[1].pixels, IntRect::from_xywh(0, 0, 2, 2).unwrap(), Rgba::BLACK);
        assert_eq!(get_pixel(&doc.flatten(), 0, 0), Some(Rgba::BLACK));
        assert_eq!(get_pixel(&doc.flatten(), 3, 3), Some(Rgba::WHITE));

        doc.layers[1].visible = false;
        assert_eq!(get_pixel(&doc.flatten(), 0, 0), Some(Rgba::WHITE));
        doc.layers[1].visible = true;

        doc.duplicate_layer(1);
        assert_eq!(doc.layers.len(), 3);
        assert_eq!(doc.active, 2);
        assert!(doc.delete_layer(2));
        assert_eq!(doc.active, 1);

        assert!(doc.merge_down(1));
        assert_eq!(doc.layers.len(), 1);
        assert_eq!(get_pixel(&doc.layers[0].pixels, 0, 0), Some(Rgba::BLACK));
        assert!(!doc.delete_layer(0), "last layer must remain");
    }

    #[test]
    fn layer_opacity_and_blend() {
        let mut doc = Document::new(2, 2, Rgba::WHITE);
        doc.add_layer();
        doc.layers[1].pixels.fill(Rgba::BLACK.to_skia());
        doc.layers[1].opacity = 0.5;
        let c = get_pixel(&doc.flatten(), 0, 0).unwrap();
        assert!((126..=129).contains(&c.r), "got {c:?}");

        doc.layers[1].opacity = 1.0;
        doc.layers[1].pixels.fill(Rgba::rgb(255, 0, 0).to_skia());
        doc.layers[1].blend = BlendMode::Multiply;
        assert_eq!(get_pixel(&doc.flatten(), 1, 1), Some(Rgba::rgb(255, 0, 0)));
    }

    #[test]
    fn move_layer_reorders() {
        let mut doc = Document::new(2, 2, Rgba::WHITE);
        doc.add_layer();
        doc.add_layer();
        let ids: Vec<_> = doc.layers.iter().map(|l| l.id).collect();
        assert!(doc.move_layer(2, 0));
        assert_eq!(doc.layers[0].id, ids[2]);
        assert_eq!(doc.active, 0);
    }

    #[test]
    fn resize_canvas_pads_with_background() {
        let mut doc = Document::new(2, 2, Rgba::BLACK);
        doc.add_layer();
        doc.resize_canvas(4, 3, 1, 1, Rgba::WHITE);
        assert_eq!((doc.width(), doc.height()), (4, 3));
        let flat = doc.flatten();
        assert_eq!(get_pixel(&flat, 0, 0), Some(Rgba::WHITE));
        assert_eq!(get_pixel(&flat, 1, 1), Some(Rgba::BLACK));
        assert_eq!(get_pixel(&flat, 3, 2), Some(Rgba::WHITE));
        assert_eq!(get_pixel(&doc.layers[1].pixels, 0, 0), Some(Rgba::TRANSPARENT));
    }

    #[test]
    fn composite_with_override() {
        let doc = Document::new(4, 4, Rgba::WHITE);
        let region = IntRect::from_xywh(1, 1, 2, 2).unwrap();
        let over = raster::filled(2, 2, Rgba::BLACK);
        let mut out = raster::new_pixmap(2, 2);
        doc.composite_into(region, &mut out, Some(&over));
        assert_eq!(get_pixel(&out, 0, 0), Some(Rgba::BLACK));
        doc.composite_into(region, &mut out, None);
        assert_eq!(get_pixel(&out, 0, 0), Some(Rgba::WHITE));
    }
}
