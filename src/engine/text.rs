// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Text layout and rasterisation for the Text tool, using the system's fonts.

use std::sync::Arc;

use cosmic_text::{
    Align, Attrs, Buffer, Color, Cursor, Family, FontSystem, Metrics, Shaping, Style, SwashCache, UnderlineStyle, Weight, Wrap,
    fontdb,
};
use tiny_skia::{IntRect, Pixmap, Rect};

use super::color::Rgba;
use super::raster;

/// The bundled UI font, always available even on systems with few fonts installed.
pub const BUNDLED_FONT: &[u8] = include_bytes!("../../assets/fonts/selawk.ttf");
pub const BUNDLED_FAMILY: &str = "Selawik";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, PartialEq, Debug)]
pub struct TextStyle {
    pub family: String,
    /// Font size in points (96 DPI, as Paint uses).
    pub size_pt: f32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub align: TextAlign,
    pub color: Rgba,
    /// Opaque background ("Fill background"), or transparent when `None`.
    pub background: Option<Rgba>,
}

impl TextStyle {
    pub fn size_px(&self) -> f32 {
        self.size_pt * 96.0 / 72.0
    }
}

/// Converts a byte offset in multi-line text into a cosmic-text cursor.
fn to_cursor(text: &str, index: usize) -> Cursor {
    let index = index.min(text.len());
    let before = &text[..index];
    let line = before.matches('\n').count();
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    Cursor::new(line, index - line_start)
}

/// Converts a cosmic-text cursor back into a byte offset.
fn from_cursor(text: &str, cursor: Cursor) -> usize {
    let mut offset = 0;
    for (i, line) in text.split('\n').enumerate() {
        if i == cursor.line {
            return offset + cursor.index.min(line.len());
        }
        offset += line.len() + 1;
    }
    text.len()
}

pub struct TextEngine {
    fonts: FontSystem,
    cache: SwashCache,
    families: Vec<String>,
}

impl TextEngine {
    /// Loads the system fonts. This can take a moment, so do it off the UI thread.
    pub fn new() -> Self {
        Self::with_fonts(true)
    }

    /// Only the bundled font; quick to create, used by tests.
    pub fn bundled_only() -> Self {
        Self::with_fonts(false)
    }

    fn with_fonts(system: bool) -> Self {
        let mut db = fontdb::Database::new();
        if system {
            db.load_system_fonts();
        }
        db.load_font_source(fontdb::Source::Binary(Arc::new(BUNDLED_FONT)));
        let mut families: Vec<String> = db
            .faces()
            .flat_map(|f| f.families.first().map(|(name, _)| name.clone()))
            .filter(|name| !name.starts_with('.'))
            .collect();
        families.sort_by_key(|f| f.to_lowercase());
        families.dedup();
        let locale = sys_locale();
        let fonts = FontSystem::new_with_locale_and_db(locale, db);
        Self {
            fonts,
            cache: SwashCache::new(),
            families,
        }
    }

    pub fn families(&self) -> &[String] {
        &self.families
    }

    pub fn has_family(&self, family: &str) -> bool {
        self.families.iter().any(|f| f.eq_ignore_ascii_case(family))
    }

    /// The best default text font available, preferring the Windows 11 Paint default.
    pub fn default_family(&self) -> String {
        for candidate in ["Segoe UI", "Calibri", "Arial", "Helvetica Neue", "Helvetica", "DejaVu Sans", "Noto Sans"] {
            if let Some(f) = self.families.iter().find(|f| f.eq_ignore_ascii_case(candidate)) {
                return f.clone();
            }
        }
        BUNDLED_FAMILY.to_owned()
    }

    fn buffer(&mut self, text: &str, style: &TextStyle, width: f32) -> Buffer {
        let px = style.size_px().max(1.0);
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(px, (px * 1.25).ceil()));
        buffer.set_wrap(Wrap::WordOrGlyph);
        buffer.set_size(Some(width.max(1.0)), None);
        let mut attrs = Attrs::new()
            .family(Family::Name(&style.family))
            .weight(if style.bold { Weight::BOLD } else { Weight::NORMAL })
            .style(if style.italic { Style::Italic } else { Style::Normal });
        if style.underline {
            attrs = attrs.underline(UnderlineStyle::Single);
        }
        if style.strikethrough {
            attrs = attrs.strikethrough();
        }
        let align = match style.align {
            TextAlign::Left => Align::Left,
            TextAlign::Center => Align::Center,
            TextAlign::Right => Align::Right,
        };
        buffer.set_text(text, &attrs, Shaping::Advanced, Some(align));
        buffer.shape_until_scroll(&mut self.fonts, false);
        buffer
    }

    /// Height of the laid-out text, in pixels.
    pub fn content_height(&mut self, text: &str, style: &TextStyle, width: f32) -> f32 {
        let buffer = self.buffer(text, style, width);
        let line_h = buffer.metrics().line_height;
        buffer.layout_runs().map(|r| r.line_top + r.line_height).fold(line_h, f32::max)
    }

    /// Height of one line of text in this style.
    pub fn line_height(&self, style: &TextStyle) -> f32 {
        (style.size_px().max(1.0) * 1.25).ceil()
    }

    /// Caret position (x, top y) relative to the text box for a byte offset.
    pub fn caret(&mut self, text: &str, style: &TextStyle, width: f32, index: usize) -> (f32, f32) {
        let buffer = self.buffer(text, style, width);
        let cursor = to_cursor(text, index);
        buffer.cursor_position(&cursor).unwrap_or_else(|| {
            let line_h = buffer.metrics().line_height;
            let x = match style.align {
                TextAlign::Left => 0.0,
                TextAlign::Center => width / 2.0,
                TextAlign::Right => width,
            };
            (x, cursor.line as f32 * line_h)
        })
    }

    /// Byte offset closest to a point relative to the text box.
    pub fn hit(&mut self, text: &str, style: &TextStyle, width: f32, x: f32, y: f32) -> usize {
        let buffer = self.buffer(text, style, width);
        match buffer.hit(x, y) {
            Some(cursor) => from_cursor(text, cursor),
            None => text.len(),
        }
    }

    /// Highlight rectangles (relative to the box) for the byte range `start..end`.
    pub fn selection_rects(&mut self, text: &str, style: &TextStyle, width: f32, start: usize, end: usize) -> Vec<Rect> {
        let buffer = self.buffer(text, style, width);
        let (a, b) = (to_cursor(text, start.min(end)), to_cursor(text, start.max(end)));
        let mut rects = Vec::new();
        for run in buffer.layout_runs() {
            for (x, w) in run.highlight(a, b) {
                if let Some(r) = Rect::from_xywh(x, run.line_top, w.max(1.0), run.line_height) {
                    rects.push(r);
                }
            }
        }
        rects
    }

    /// Renders the text into `target` with the box's top-left at `(x, y)`.
    /// The background (if any) fills the whole `width` × `height` box.
    pub fn render(
        &mut self,
        target: &mut Pixmap,
        text: &str,
        style: &TextStyle,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Option<IntRect> {
        let area = raster::clamp_to(IntRect::from_xywh(x, y, width.max(1), height.max(1))?, target.width(), target.height())?;
        if let Some(bg) = style.background {
            let mut paint = tiny_skia::Paint::default();
            paint.set_color(bg.to_skia());
            target.fill_rect(area.to_rect(), &paint, tiny_skia::Transform::identity(), None);
        }
        let mut buffer = self.buffer(text, style, width as f32);
        let c = style.color;
        let (tw, th) = (target.width() as i32, target.height() as i32);
        let clip = area;
        buffer.draw(&mut self.fonts, &mut self.cache, Color::rgba(c.r, c.g, c.b, 255), |gx, gy, gw, gh, color| {
            let alpha = color.a();
            if alpha == 0 {
                return;
            }
            for py in gy..gy + gh as i32 {
                for px in gx..gx + gw as i32 {
                    let (dx, dy) = (x + px, y + py);
                    if dx < clip.left() || dy < clip.top() || dx >= clip.right() || dy >= clip.bottom() || dx >= tw || dy >= th {
                        continue;
                    }
                    blend_pixel(target, dx, dy, Rgba::new(color.r(), color.g(), color.b(), alpha));
                }
            }
        });
        Some(area)
    }
}

impl Default for TextEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn sys_locale() -> String {
    std::env::var("LANG")
        .ok()
        .and_then(|l| l.split('.').next().map(|s| s.replace('_', "-")))
        .filter(|l| !l.is_empty() && l != "C")
        .unwrap_or_else(|| "en-US".to_owned())
}

/// Source-over blend of a straight-alpha colour onto one pixel.
fn blend_pixel(p: &mut Pixmap, x: i32, y: i32, c: Rgba) {
    let w = p.width() as usize;
    let idx = y as usize * w + x as usize;
    let dst = p.pixels()[idx];
    let a = c.a as u32;
    let inv = 255 - a;
    let mix = |s: u8, d: u8| ((s as u32 * a + d as u32 * inv + 127) / 255) as u8;
    let out_a = (a + (dst.alpha() as u32 * inv + 127) / 255).min(255) as u8;
    let r = mix(c.r, dst.red()).min(out_a);
    let g = mix(c.g, dst.green()).min(out_a);
    let b = mix(c.b, dst.blue()).min(out_a);
    if let Some(px) = tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, out_a) {
        p.pixels_mut()[idx] = px;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::raster::{content_bounds, filled, get_pixel};

    fn style() -> TextStyle {
        TextStyle {
            family: BUNDLED_FAMILY.into(),
            size_pt: 24.0,
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            align: TextAlign::Left,
            color: Rgba::BLACK,
            background: None,
        }
    }

    #[test]
    fn cursor_conversion_round_trips() {
        let text = "ab\ncde\n\nf";
        for i in 0..=text.len() {
            assert_eq!(from_cursor(text, to_cursor(text, i)), i, "index {i}");
        }
        assert_eq!(to_cursor(text, 4), Cursor::new(1, 1));
    }

    #[test]
    fn renders_text_inside_box() {
        let mut engine = TextEngine::bundled_only();
        let mut target = raster::new_pixmap(300, 100);
        engine.render(&mut target, "Hello", &style(), 10, 10, 200, 50).unwrap();
        let bounds = content_bounds(&target).expect("text drew something");
        assert!(bounds.left() >= 10 && bounds.top() >= 10 && bounds.right() <= 210 && bounds.bottom() <= 60, "{bounds:?}");
        assert!(bounds.width() > 40, "{bounds:?}");
    }

    #[test]
    fn background_fills_box() {
        let mut engine = TextEngine::bundled_only();
        let mut target = filled(100, 100, Rgba::BLACK);
        let mut s = style();
        s.background = Some(Rgba::WHITE);
        engine.render(&mut target, "", &s, 10, 10, 30, 20).unwrap();
        assert_eq!(get_pixel(&target, 20, 20), Some(Rgba::WHITE));
        assert_eq!(get_pixel(&target, 50, 50), Some(Rgba::BLACK));
    }

    #[test]
    fn wraps_long_text() {
        let mut engine = TextEngine::bundled_only();
        let one = engine.content_height("word", &style(), 80.0);
        let many = engine.content_height("word word word word word", &style(), 80.0);
        assert!(many > one * 2.0, "{one} vs {many}");
    }

    #[test]
    fn caret_and_hit_agree() {
        let mut engine = TextEngine::bundled_only();
        let text = "Hello\nWorld";
        let (x, y) = engine.caret(text, &style(), 300.0, 8);
        assert!(x > 0.0 && y > 0.0);
        let back = engine.hit(text, &style(), 300.0, x + 1.0, y + 2.0);
        assert_eq!(back, 8);
        assert_eq!(engine.caret(text, &style(), 300.0, 0), (0.0, 0.0));
    }

    #[test]
    fn bundled_family_is_listed() {
        let engine = TextEngine::bundled_only();
        assert!(engine.has_family(BUNDLED_FAMILY));
        assert_eq!(engine.default_family(), BUNDLED_FAMILY);
    }
}
