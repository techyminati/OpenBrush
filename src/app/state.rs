// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Plain data describing the current tool, colours and view.

use crate::engine::brush::BrushKind;
use crate::ui::icons::Icon;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Tool {
    Select,
    Pencil,
    Fill,
    Text,
    Eraser,
    ColorPicker,
    Magnifier,
    Brush,
    Shape,
}

impl Tool {
    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Selection",
            Tool::Pencil => "Pencil",
            Tool::Fill => "Fill with color",
            Tool::Text => "Text",
            Tool::Eraser => "Eraser",
            Tool::ColorPicker => "Color picker",
            Tool::Magnifier => "Magnifier",
            Tool::Brush => "Brushes",
            Tool::Shape => "Shapes",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Tool::Select => Icon::SelectRect,
            Tool::Pencil => Icon::Pencil,
            Tool::Fill => Icon::Fill,
            Tool::Text => Icon::Text,
            Tool::Eraser => Icon::Eraser,
            Tool::ColorPicker => Icon::Picker,
            Tool::Magnifier => Icon::Magnifier,
            Tool::Brush => Icon::Brush,
            Tool::Shape => Icon::Outline,
        }
    }

    /// Which size setting the Size dropdown edits for this tool.
    pub fn size_kind(self) -> Option<SizeKind> {
        match self {
            Tool::Pencil => Some(SizeKind::Pencil),
            Tool::Brush => Some(SizeKind::Brush),
            Tool::Eraser => Some(SizeKind::Eraser),
            Tool::Shape => Some(SizeKind::Shape),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum SelectMode {
    Rectangle,
    FreeForm,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum ColorSlot {
    Primary,
    Secondary,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum SizeKind {
    Pencil,
    Brush,
    Eraser,
    Shape,
}

impl SizeKind {
    /// Quick-pick sizes shown in the Size dropdown, as Paint offers.
    pub fn presets(self) -> &'static [u32] {
        match self {
            SizeKind::Pencil => &[1, 2, 3, 4],
            SizeKind::Brush => &[1, 3, 5, 8, 12, 20],
            SizeKind::Eraser => &[4, 6, 8, 10, 16, 24],
            SizeKind::Shape => &[1, 3, 5, 8, 12],
        }
    }

    pub fn max(self) -> u32 {
        match self {
            SizeKind::Pencil => 50,
            _ => 100,
        }
    }
}

/// Line widths for each kind of tool, in pixels.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Sizes {
    pub pencil: u32,
    pub brush: u32,
    pub eraser: u32,
    pub shape: u32,
}

impl Default for Sizes {
    fn default() -> Self {
        Self {
            pencil: 1,
            brush: 5,
            eraser: 8,
            shape: 3,
        }
    }
}

impl Sizes {
    pub fn get(&self, kind: SizeKind) -> u32 {
        match kind {
            SizeKind::Pencil => self.pencil,
            SizeKind::Brush => self.brush,
            SizeKind::Eraser => self.eraser,
            SizeKind::Shape => self.shape,
        }
    }

    pub fn set(&mut self, kind: SizeKind, value: u32) {
        let value = value.clamp(1, kind.max());
        match kind {
            SizeKind::Pencil => self.pencil = value,
            SizeKind::Brush => self.brush = value,
            SizeKind::Eraser => self.eraser = value,
            SizeKind::Shape => self.shape = value,
        }
    }
}

/// Zoom presets, as in Paint's zoom menu (fractions of 100%).
pub const ZOOM_LEVELS: [f32; 11] = [0.125, 0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
pub const MIN_ZOOM: f32 = 0.01;
pub const MAX_ZOOM: f32 = 8.0;

pub fn next_zoom_in(zoom: f32) -> f32 {
    ZOOM_LEVELS.iter().copied().find(|&z| z > zoom + 1e-4).unwrap_or(MAX_ZOOM)
}

pub fn next_zoom_out(zoom: f32) -> f32 {
    ZOOM_LEVELS.iter().rev().copied().find(|&z| z < zoom - 1e-4).unwrap_or(MIN_ZOOM.max(zoom / 2.0))
}

#[derive(Clone, Debug)]
pub struct View {
    /// Screen points per image pixel.
    pub zoom: f32,
    /// Scroll offset of the canvas, in screen points.
    pub scroll: egui::Vec2,
    pub show_rulers: bool,
    pub show_grid: bool,
    pub show_status_bar: bool,
    pub show_thumbnail: bool,
    pub show_layers: bool,
    pub fullscreen: bool,
    /// Fit the canvas to the window on the next frame.
    pub fit_pending: bool,
    /// Keep fitting the canvas as the window resizes, until the user zooms by hand.
    pub auto_fit: bool,
    /// Viewport size the last automatic fit was computed for.
    pub fitted_for: egui::Vec2,
}

impl Default for View {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            scroll: egui::Vec2::ZERO,
            show_rulers: false,
            show_grid: false,
            show_status_bar: true,
            show_thumbnail: false,
            show_layers: false,
            fullscreen: false,
            fit_pending: true,
            auto_fit: true,
            fitted_for: egui::Vec2::ZERO,
        }
    }
}

pub fn default_brush() -> BrushKind {
    BrushKind::Brush
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_steps() {
        assert_eq!(next_zoom_in(1.0), 2.0);
        assert_eq!(next_zoom_in(0.6), 1.0);
        assert_eq!(next_zoom_in(8.0), 8.0);
        assert_eq!(next_zoom_out(1.0), 0.5);
        assert_eq!(next_zoom_out(0.125), 0.0625);
        assert_eq!(next_zoom_out(2.5), 2.0);
    }

    #[test]
    fn sizes_clamp() {
        let mut s = Sizes::default();
        s.set(SizeKind::Pencil, 500);
        assert_eq!(s.pencil, 50);
        s.set(SizeKind::Brush, 0);
        assert_eq!(s.brush, 1);
    }
}
