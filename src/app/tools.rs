// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! What each tool does with the pointer, and committing finished edits to the image.

use egui::PointerButton;
use tiny_skia::{IntRect, Pixmap, PixmapPaint, Point, Rect};

use super::state::{ColorSlot, SelectMode, Tool, next_zoom_in, next_zoom_out};
use super::PaintApp;
use crate::engine::brush::{self, Stroke, StrokeKind};
use crate::engine::color::Rgba;
use crate::engine::document::LayerId;
use crate::engine::fill;
use crate::engine::raster;
use crate::engine::selection::Selection;
use crate::engine::shapes::{self, Geometry, ShapeKind, ShapeStyle};

/// A pointer interaction in progress on the canvas.
pub(crate) enum Gesture {
    None,
    Stroke { stroke: Stroke, area: Option<IntRect> },
    Marquee { start: Point, end: Point },
    Lasso { points: Vec<Point> },
    MoveSelection { last: Point, moved: (f32, f32) },
    ResizeSelection { handle: Handle, start: Rect, grab: Point },
    CreateShape { start: Point },
    MoveShape { last: Point },
    ResizeShape { handle: Handle, start: Geometry, grab: Point },
    BendCurve,
    PlacePolygonPoint,
    CreateText { start: Point, end: Point },
    MoveText { last: Point },
    ResizeText { handle: Handle, start: Rect, grab: Point },
    SelectText,
    ResizeCanvas { handle: Handle, width: u32, height: u32 },
    Pan { last: egui::Pos2 },
}

impl Gesture {
    pub fn is_none(&self) -> bool {
        matches!(self, Gesture::None)
    }
}

/// Resize handles around a box (and the two ends of a line).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Handle {
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
    Start,
    End,
}

impl Handle {
    pub const BOX: [Handle; 8] = [Handle::NW, Handle::N, Handle::NE, Handle::E, Handle::SE, Handle::S, Handle::SW, Handle::W];

    /// Position of the handle on `r`.
    pub fn position(self, r: Rect) -> Point {
        let (cx, cy) = ((r.left() + r.right()) / 2.0, (r.top() + r.bottom()) / 2.0);
        let (x, y) = match self {
            Handle::NW => (r.left(), r.top()),
            Handle::N => (cx, r.top()),
            Handle::NE => (r.right(), r.top()),
            Handle::E => (r.right(), cy),
            Handle::SE => (r.right(), r.bottom()),
            Handle::S => (cx, r.bottom()),
            Handle::SW => (r.left(), r.bottom()),
            Handle::W => (r.left(), cy),
            Handle::Start | Handle::End => (cx, cy),
        };
        Point::from_xy(x, y)
    }

    pub fn cursor(self) -> egui::CursorIcon {
        use egui::CursorIcon as C;
        match self {
            Handle::N | Handle::S => C::ResizeVertical,
            Handle::E | Handle::W => C::ResizeHorizontal,
            Handle::NE | Handle::SW => C::ResizeNeSw,
            Handle::NW | Handle::SE => C::ResizeNwSe,
            Handle::Start | Handle::End => C::Crosshair,
        }
    }

    /// `r` with this handle dragged by `(dx, dy)`. The result may be flipped; it is normalised.
    pub fn drag(self, r: Rect, dx: f32, dy: f32) -> Option<Rect> {
        let (mut l, mut t, mut rt, mut b) = (r.left(), r.top(), r.right(), r.bottom());
        match self {
            Handle::N => t += dy,
            Handle::S => b += dy,
            Handle::E => rt += dx,
            Handle::W => l += dx,
            Handle::NE => {
                t += dy;
                rt += dx;
            }
            Handle::NW => {
                t += dy;
                l += dx;
            }
            Handle::SE => {
                b += dy;
                rt += dx;
            }
            Handle::SW => {
                b += dy;
                l += dx;
            }
            Handle::Start | Handle::End => {}
        }
        let (l, rt) = (l.min(rt), l.max(rt));
        let (t, b) = (t.min(b), t.max(b));
        Rect::from_ltrb(l, t, rt.max(l + 1.0), b.max(t + 1.0))
    }
}

/// A selection on a particular layer, with the layer's pixels from before it was lifted
/// (so the whole move can be undone as one step).
pub(crate) struct ActiveSelection {
    pub sel: Selection,
    pub layer: LayerId,
    pub before: Option<Pixmap>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ShapeStage {
    /// Finished drawing; can be moved and resized until committed.
    Editing,
    /// The curve tool's two "bend" steps.
    CurveBend1,
    CurveBend2,
    /// Polygon still accepting points.
    PolygonOpen,
}

pub(crate) struct ShapeEdit {
    pub geometry: Geometry,
    /// Drawn with the secondary button: outline and fill colours swap.
    pub swap_colors: bool,
    pub stage: ShapeStage,
    pub rendered: Option<(Geometry, ShapeStyle)>,
}

pub(crate) struct TextBox {
    /// Box position in document pixels.
    pub rect: Rect,
    pub text: String,
    pub cursor: usize,
    pub anchor: usize,
    pub swap_colors: bool,
    pub rendered: Option<String>,
}

impl TextBox {
    pub fn selection(&self) -> (usize, usize) {
        (self.cursor.min(self.anchor), self.cursor.max(self.anchor))
    }
}

pub(crate) fn constrain_square(start: Point, end: Point) -> Point {
    let (dx, dy) = (end.x - start.x, end.y - start.y);
    let side = dx.abs().max(dy.abs());
    Point::from_xy(start.x + side * dx.signum(), start.y + side * dy.signum())
}

/// Snaps a line to the nearest 45°.
pub(crate) fn constrain_angle(start: Point, end: Point) -> Point {
    let (dx, dy) = (end.x - start.x, end.y - start.y);
    let len = (dx * dx + dy * dy).sqrt();
    let angle = dy.atan2(dx);
    let step = std::f32::consts::FRAC_PI_4;
    let snapped = (angle / step).round() * step;
    Point::from_xy(start.x + len * snapped.cos(), start.y + len * snapped.sin())
}

fn pixel(p: Point) -> (i32, i32) {
    (p.x.floor() as i32, p.y.floor() as i32)
}

impl PaintApp {
    // ---------------------------------------------------------------- tool switching

    /// Selects a tool, finishing any edit that belongs to the previous one.
    pub fn set_tool(&mut self, tool: Tool) {
        if tool != self.tool {
            self.commit_all();
            if tool == Tool::ColorPicker || tool == Tool::Magnifier {
                self.previous_tool = self.tool;
            }
            self.tool = tool;
        }
    }

    /// Commits any pending shape, text box or floating selection.
    pub fn commit_all(&mut self) {
        self.commit_shape();
        self.commit_text();
        self.commit_selection();
    }

    pub(crate) fn color_for(&self, button: PointerButton) -> Rgba {
        if button == PointerButton::Secondary { self.color2 } else { self.color1 }
    }

    pub fn set_color(&mut self, slot: ColorSlot, color: Rgba) {
        match slot {
            ColorSlot::Primary => self.color1 = color,
            ColorSlot::Secondary => self.color2 = color,
        }
    }

    // ---------------------------------------------------------------- drafts

    pub(crate) fn fresh_draft(&mut self) -> &mut Pixmap {
        let (w, h) = (self.doc.width(), self.doc.height());
        let reuse = self.draft.as_ref().is_some_and(|d| d.width() == w && d.height() == h);
        if !reuse {
            self.draft = Some(raster::new_pixmap(w, h));
        } else if let Some(area) = self.draft_area.take() {
            let draft = self.draft.as_mut().expect("checked above");
            raster::fill_region(draft, area, Rgba::TRANSPARENT);
        }
        self.draft_area = None;
        self.draft.as_mut().expect("draft exists")
    }

    pub(crate) fn discard_draft(&mut self) {
        if let Some(area) = self.draft_area.take() {
            self.invalidate(area);
        }
        self.draft = None;
        self.draft_paint = PixmapPaint::default();
    }

    /// Composites the draft into the active layer as one undoable step.
    pub(crate) fn commit_draft(&mut self, label: &str) {
        let (Some(draft), Some(area)) = (self.draft.take(), self.draft_area.take()) else {
            self.draft = None;
            return;
        };
        let Some(area) = raster::clamp_to(area, self.doc.width(), self.doc.height()) else {
            return;
        };
        let paint = std::mem::take(&mut self.draft_paint);
        let layer = self.doc.active_layer_mut();
        let before = raster::copy_region(&layer.pixels, area);
        brush::apply_draft(&mut layer.pixels, &draft, area, &paint);
        let id = layer.id;
        self.history.push_pixels(label, id, area, before);
        self.invalidate(area);
    }

    // ---------------------------------------------------------------- freehand strokes

    pub(crate) fn stroke_kind(&self) -> Option<StrokeKind> {
        match self.tool {
            Tool::Pencil => Some(StrokeKind::Pencil),
            Tool::Eraser => Some(StrokeKind::Eraser),
            Tool::Brush => Some(StrokeKind::Brush(self.brush)),
            _ => None,
        }
    }

    pub(crate) fn stroke_size(&self) -> f32 {
        match self.tool {
            Tool::Pencil => self.sizes.pencil as f32,
            Tool::Eraser => self.sizes.eraser as f32,
            _ => self.sizes.brush as f32,
        }
    }

    pub(crate) fn begin_stroke(&mut self, button: PointerButton, at: Point) {
        let Some(kind) = self.stroke_kind() else {
            return;
        };
        let color = self.color_for(button);
        let seed = self.next_seed();
        let mut stroke = Stroke::new(kind, color, self.stroke_size(), seed);
        self.draft_paint = stroke.composite_paint();
        let draft = self.fresh_draft();
        let area = stroke.add_point(draft, at.x, at.y);
        self.draft_area = area;
        if let Some(a) = area {
            self.invalidate(a);
        }
        self.gesture = Gesture::Stroke { stroke, area };
    }

    pub(crate) fn continue_stroke(&mut self, at: Point, seconds: f32) {
        let Gesture::Stroke { stroke, area } = &mut self.gesture else {
            return;
        };
        let Some(draft) = self.draft.as_mut() else {
            return;
        };
        let mut dirty = stroke.add_point(draft, at.x, at.y);
        if seconds > 0.0 {
            dirty = raster::union_opt(dirty, stroke.tick(draft, seconds));
        }
        *area = raster::union_opt(*area, dirty);
        self.draft_area = *area;
        if let Some(d) = dirty {
            self.canvas.invalidate(d);
        }
    }

    pub(crate) fn end_stroke(&mut self) {
        let Gesture::Stroke { stroke, .. } = std::mem::replace(&mut self.gesture, Gesture::None) else {
            return;
        };
        let label = match stroke.kind() {
            StrokeKind::Pencil => "Pencil",
            StrokeKind::Eraser => "Eraser",
            StrokeKind::Brush(b) => b.label(),
        };
        self.commit_draft(label);
    }

    // ---------------------------------------------------------------- fill, picker, magnifier

    pub(crate) fn fill_at(&mut self, button: PointerButton, at: Point) {
        let (x, y) = pixel(at);
        let color = self.color_for(button);
        let layer = self.doc.active_layer_mut();
        let before = layer.pixels.clone();
        if let Some(changed) = fill::flood_fill(&mut layer.pixels, x, y, color, fill::DEFAULT_TOLERANCE) {
            let id = layer.id;
            self.history.push_pixels("Fill", id, changed, raster::copy_region(&before, changed));
            self.invalidate(changed);
        }
    }

    pub(crate) fn pick_at(&mut self, button: PointerButton, at: Point) {
        let (x, y) = pixel(at);
        let Some(r) = IntRect::from_xywh(x, y, 1, 1) else {
            return;
        };
        if raster::clamp_to(r, self.doc.width(), self.doc.height()).is_none() {
            return;
        }
        let mut px = raster::new_pixmap(1, 1);
        self.doc.composite_into(r, &mut px, None);
        let Some(c) = raster::get_pixel(&px, 0, 0) else {
            return;
        };
        // Transparent areas pick as white, the colour they print and save as.
        let c = if c.a == 0 { Rgba::WHITE } else { c.with_alpha(255) };
        let slot = if button == PointerButton::Secondary { ColorSlot::Secondary } else { ColorSlot::Primary };
        self.set_color(slot, c);
        let back = self.previous_tool;
        self.set_tool(if back == Tool::ColorPicker { Tool::Pencil } else { back });
    }

    pub(crate) fn magnify_at(&mut self, button: PointerButton, at: Point, viewport: egui::Rect) {
        let zoom = if button == PointerButton::Secondary {
            next_zoom_out(self.view.zoom)
        } else {
            next_zoom_in(self.view.zoom)
        };
        self.zoom_to(zoom, Some(at), viewport);
    }

    // ---------------------------------------------------------------- shapes

    pub(crate) fn shape_style(&self, swap: bool) -> ShapeStyle {
        let (outline_color, fill_color) = if swap { (self.color2, self.color1) } else { (self.color1, self.color2) };
        ShapeStyle {
            outline: self.outline,
            fill: self.fill,
            outline_color,
            fill_color,
            width: self.sizes.shape as f32,
        }
    }

    pub(crate) fn begin_shape(&mut self, button: PointerButton, at: Point) {
        let geometry = match self.shape_kind {
            ShapeKind::Line => Geometry::Line { a: at, b: at },
            ShapeKind::Curve => Geometry::Curve { a: at, b: at, c1: at, c2: at },
            ShapeKind::Polygon => Geometry::Polygon {
                points: vec![at, at],
                closed: false,
            },
            kind => Geometry::Boxed { kind, a: at, b: at },
        };
        self.shape = Some(ShapeEdit {
            geometry,
            swap_colors: button == PointerButton::Secondary,
            stage: ShapeStage::Editing,
            rendered: None,
        });
        self.gesture = Gesture::CreateShape { start: at };
    }

    pub(crate) fn drag_new_shape(&mut self, start: Point, at: Point, constrain: bool) {
        let Some(shape) = self.shape.as_mut() else {
            return;
        };
        match &mut shape.geometry {
            Geometry::Line { b, .. } => *b = if constrain { constrain_angle(start, at) } else { at },
            Geometry::Curve { b, c1, c2, .. } => {
                *b = if constrain { constrain_angle(start, at) } else { at };
                *c2 = *b;
                *c1 = start;
            }
            Geometry::Polygon { points, .. } => {
                if let Some(last) = points.last_mut() {
                    *last = if constrain { constrain_angle(start, at) } else { at };
                }
            }
            Geometry::Boxed { b, .. } => *b = if constrain { constrain_square(start, at) } else { at },
        }
    }

    /// The pointer was released after drawing a new shape.
    pub(crate) fn finish_new_shape(&mut self) {
        let Some(shape) = self.shape.as_mut() else {
            return;
        };
        let tiny = shape.geometry.bounds().is_none_or(|b| b.width() < 1.0 && b.height() < 1.0);
        match &mut shape.geometry {
            Geometry::Curve { .. } if !tiny => shape.stage = ShapeStage::CurveBend1,
            Geometry::Polygon { .. } if !tiny => shape.stage = ShapeStage::PolygonOpen,
            Geometry::Boxed { a, b, .. } => {
                let (l, t, r, bt) = (a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y));
                *a = Point::from_xy(l, t);
                *b = Point::from_xy(r, bt);
            }
            _ => {}
        }
        if tiny {
            self.cancel_shape();
        }
    }

    /// Re-renders the shape preview if its geometry or style changed.
    pub(crate) fn refresh_shape(&mut self) {
        let Some(shape) = self.shape.as_ref() else {
            return;
        };
        let style = self.shape_style(shape.swap_colors);
        let key = (shape.geometry.clone(), style);
        if shape.rendered.as_ref() == Some(&key) {
            return;
        }
        let old = self.draft_area;
        let seed = self.seed;
        self.draft_paint = PixmapPaint::default();
        let draft = self.fresh_draft();
        let area = shapes::render(draft, &key.0, &key.1, seed);
        self.draft_area = area;
        if let Some(r) = raster::union_opt(old, area) {
            self.invalidate(r);
        }
        if let Some(shape) = self.shape.as_mut() {
            shape.rendered = Some(key);
        }
    }

    pub fn commit_shape(&mut self) {
        if self.shape.is_none() {
            return;
        }
        self.refresh_shape();
        let label = self.shape_kind.label();
        self.shape = None;
        self.commit_draft(label);
        self.next_seed();
    }

    pub fn cancel_shape(&mut self) {
        if self.shape.take().is_some() {
            self.discard_draft();
        }
    }

    // ---------------------------------------------------------------- selection

    pub(crate) fn begin_marquee(&mut self, at: Point) {
        self.commit_selection();
        self.gesture = match self.select_mode {
            SelectMode::Rectangle => Gesture::Marquee { start: at, end: at },
            SelectMode::FreeForm => Gesture::Lasso { points: vec![at] },
        };
    }

    pub(crate) fn finish_marquee(&mut self) {
        let (w, h) = (self.doc.width(), self.doc.height());
        let sel = match std::mem::replace(&mut self.gesture, Gesture::None) {
            Gesture::Marquee { start, end } => raster::rect_around(start.x, start.y, end.x, end.y, 0.0)
                .and_then(|_| {
                    // Corners snap to the nearest pixel grid line, so the size matches the drag.
                    let (x0, y0) = (start.x.min(end.x).round(), start.y.min(end.y).round());
                    let (x1, y1) = (start.x.max(end.x).round(), start.y.max(end.y).round());
                    IntRect::from_ltrb(x0 as i32, y0 as i32, x1 as i32, y1 as i32)
                })
                .and_then(|r| raster::clamp_to(r, w, h))
                .filter(|r| r.width() >= 1 && r.height() >= 1)
                .map(Selection::rect),
            Gesture::Lasso { points } => Selection::freeform(&points, w, h),
            _ => None,
        };
        self.selection = sel.map(|sel| ActiveSelection {
            sel,
            layer: self.doc.active_layer().id,
            before: None,
        });
    }

    /// Lifts the selected pixels off the layer so they can move.
    pub(crate) fn lift_selection(&mut self, duplicate: bool) {
        let Some(active) = self.selection.as_mut() else {
            return;
        };
        if active.sel.is_floating() {
            return;
        }
        let Some(index) = self.doc.layer_index(active.layer) else {
            return;
        };
        let layer = &mut self.doc.layers[index].pixels;
        active.before = Some(layer.clone());
        active.sel.lift(layer, duplicate);
        let origin = active.sel.origin;
        self.invalidate(origin);
    }

    /// Stamps a floating selection back down as one undoable step, and deselects.
    pub fn commit_selection(&mut self) {
        let Some(active) = self.selection.take() else {
            return;
        };
        let Some(index) = self.doc.layer_index(active.layer) else {
            return;
        };
        let Some(before) = active.before else {
            // Never lifted: nothing changed.
            return;
        };
        let key = self.transparent_selection.then_some(self.color2);
        let layer = &mut self.doc.layers[index];
        active.sel.draw_floating(&mut layer.pixels, (0, 0), key);
        let area = raster::union_opt(Some(active.sel.origin), active.sel.floating_area())
            .and_then(|r| raster::clamp_to(r, layer.pixels.width(), layer.pixels.height()));
        if let Some(area) = area {
            let id = layer.id;
            self.history.push_pixels("Move selection", id, area, raster::copy_region(&before, area));
            self.invalidate(area);
        }
    }

    /// Drops the selection, restoring the layer if pixels had been lifted.
    pub fn cancel_selection(&mut self) {
        let Some(active) = self.selection.take() else {
            return;
        };
        if let (Some(before), Some(index)) = (active.before, self.doc.layer_index(active.layer)) {
            self.doc.layers[index].pixels = before;
            self.invalidate_all();
        } else if let Some(area) = active.sel.floating_area() {
            self.invalidate(area);
        }
    }

    pub(crate) fn move_selection(&mut self, dx: f32, dy: f32) {
        if !self.selection.as_ref().is_some_and(|s| s.sel.is_floating()) {
            self.lift_selection(false);
        }
        let Some(active) = self.selection.as_mut() else {
            return;
        };
        let old = active.sel.floating_area();
        if let Some(f) = active.sel.floating.as_mut() {
            if let Some(r) = Rect::from_xywh(f.rect.x() + dx, f.rect.y() + dy, f.rect.width(), f.rect.height()) {
                f.rect = r;
            }
        }
        let new = active.sel.floating_area();
        if let Some(r) = raster::union_opt(old, new) {
            self.invalidate(r);
        }
    }

    pub(crate) fn resize_selection_to(&mut self, rect: Rect) {
        let Some(active) = self.selection.as_mut() else {
            return;
        };
        let old = active.sel.floating_area();
        if let Some(f) = active.sel.floating.as_mut() {
            f.rect = rect;
        }
        let new = active.sel.floating_area();
        if let Some(r) = raster::union_opt(old, new) {
            self.invalidate(r);
        }
    }

    // ---------------------------------------------------------------- text

    pub(crate) fn begin_text_box(&mut self, at: Point) {
        self.gesture = Gesture::CreateText { start: at, end: at };
    }

    pub(crate) fn finish_text_box(&mut self, start: Point, end: Point, swap: bool) {
        if self.text_style.family.is_empty() {
            self.text_style.family = self.fonts.get().default_family();
        }
        let line = self.fonts.get().line_height(&self.text_style);
        let (l, t) = (start.x.min(end.x).floor(), start.y.min(end.y).floor());
        let (mut w, mut h) = ((start.x - end.x).abs().ceil(), (start.y - end.y).abs().ceil());
        if w < 8.0 || h < 8.0 {
            // A click (rather than a drag) makes a default-sized box, as in Paint.
            w = (line * 12.0).max(150.0);
            h = line + 8.0;
        }
        let w = w.min(self.doc.width() as f32 - l).max(8.0);
        let h = h.max(line + 4.0);
        let Some(rect) = Rect::from_xywh(l, t, w, h) else {
            return;
        };
        self.text = Some(TextBox {
            rect,
            text: String::new(),
            cursor: 0,
            anchor: 0,
            swap_colors: swap,
            rendered: None,
        });
        self.refresh_text(true);
    }

    pub(crate) fn current_text_style(&self, swap: bool) -> crate::engine::text::TextStyle {
        let (fg, bg) = if swap { (self.color2, self.color1) } else { (self.color1, self.color2) };
        let mut style = self.text_style.clone();
        style.color = fg;
        style.background = self.text_background.then_some(bg);
        style
    }

    /// Re-renders the text preview, growing the box downwards to fit the text.
    pub(crate) fn refresh_text(&mut self, force: bool) {
        let Some(tb) = self.text.as_ref() else {
            return;
        };
        let style = self.current_text_style(tb.swap_colors);
        let key = format!("{:?}|{:?}|{}|{}", tb.rect, style, tb.text, self.doc.width());
        if !force && tb.rendered.as_deref() == Some(key.as_str()) {
            return;
        }
        let text = tb.text.clone();
        let rect = tb.rect;
        let needed = self.fonts.get().content_height(&text, &style, rect.width());
        let rect = if needed + 4.0 > rect.height() {
            Rect::from_xywh(rect.x(), rect.y(), rect.width(), needed + 4.0).unwrap_or(rect)
        } else {
            rect
        };
        let old = self.draft_area;
        self.draft_paint = PixmapPaint::default();
        let mut draft = std::mem::take(&mut self.draft);
        let target = match draft.as_mut() {
            Some(d) if d.width() == self.doc.width() && d.height() == self.doc.height() => {
                if let Some(a) = old {
                    raster::fill_region(d, a, Rgba::TRANSPARENT);
                }
                d
            }
            _ => draft.insert(raster::new_pixmap(self.doc.width(), self.doc.height())),
        };
        let area = self.fonts.get().render(
            target,
            &text,
            &style,
            rect.x() as i32,
            rect.y() as i32,
            rect.width() as u32,
            rect.height() as u32,
        );
        self.draft = draft;
        self.draft_area = area;
        if let Some(r) = raster::union_opt(old, area) {
            self.invalidate(r);
        }
        if let Some(tb) = self.text.as_mut() {
            tb.rect = rect;
            tb.rendered = Some(key);
        }
    }

    pub fn commit_text(&mut self) {
        let Some(tb) = self.text.take() else {
            return;
        };
        if tb.text.trim().is_empty() && !self.text_background {
            self.discard_draft();
            return;
        }
        self.text = Some(tb);
        self.refresh_text(false);
        self.text = None;
        self.commit_draft("Text");
    }

    pub(crate) fn text_hit(&mut self, at: Point) -> Option<usize> {
        let tb = self.text.as_ref()?;
        let style = self.current_text_style(tb.swap_colors);
        let (text, rect) = (tb.text.clone(), tb.rect);
        Some(self.fonts.get().hit(&text, &style, rect.width(), at.x - rect.x(), at.y - rect.y()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_drag_flips() {
        let r = Rect::from_xywh(10.0, 10.0, 20.0, 20.0).unwrap();
        let out = Handle::E.drag(r, -30.0, 0.0).unwrap();
        assert_eq!((out.left(), out.right()), (0.0, 10.0));
        let out = Handle::SE.drag(r, 5.0, 5.0).unwrap();
        assert_eq!((out.width(), out.height()), (25.0, 25.0));
    }

    #[test]
    fn constraints() {
        let s = Point::from_xy(0.0, 0.0);
        let sq = constrain_square(s, Point::from_xy(10.0, -4.0));
        assert_eq!((sq.x, sq.y), (10.0, -10.0));
        let l = constrain_angle(s, Point::from_xy(10.0, 1.0));
        assert!((l.y).abs() < 1e-4);
        let d = constrain_angle(s, Point::from_xy(10.0, 9.0));
        assert!((d.x - d.y).abs() < 1e-3);
    }
}
