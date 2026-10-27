// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! The workspace around the canvas: layout, zoom, scrolling, pointer input and overlays.

use egui::{Align2, Color32, CornerRadius, CursorIcon, Id, PointerButton, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use tiny_skia::Point;

use super::PaintApp;
use super::state::{MAX_ZOOM, MIN_ZOOM, SelectMode, Tool};
use super::tools::{Gesture, Handle, ShapeStage};
use crate::engine::shapes::Geometry;
use crate::ui::theme::Palette;
use crate::ui::widgets;

/// Space kept around the canvas when it is larger than the window.
const MARGIN: f32 = 24.0;
const RULER: f32 = 22.0;
const HANDLE: f32 = 7.0;
const SCROLLBAR: f32 = 8.0;

impl PaintApp {
    /// Top-left corner of the canvas on screen.
    pub(crate) fn canvas_origin(&self, viewport: Rect) -> Pos2 {
        let content = vec2(self.doc.width() as f32, self.doc.height() as f32) * self.view.zoom;
        let axis = |min: f32, len: f32, content: f32, scroll: f32| {
            if content + 2.0 * MARGIN <= len {
                min + ((len - content) / 2.0).round()
            } else {
                min + MARGIN - scroll
            }
        };
        pos2(
            axis(viewport.min.x, viewport.width(), content.x, self.view.scroll.x),
            axis(viewport.min.y, viewport.height(), content.y, self.view.scroll.y),
        )
    }

    fn max_scroll(&self, viewport: Rect) -> Vec2 {
        let content = vec2(self.doc.width() as f32, self.doc.height() as f32) * self.view.zoom;
        vec2(
            (content.x + 2.0 * MARGIN - viewport.width()).max(0.0),
            (content.y + 2.0 * MARGIN - viewport.height()).max(0.0),
        )
    }

    fn clamp_scroll(&mut self, viewport: Rect) {
        let max = self.max_scroll(viewport);
        self.view.scroll = self.view.scroll.clamp(Vec2::ZERO, max);
    }

    /// The zoom that fits the whole canvas in the viewport (never above 100%).
    pub(crate) fn fit_zoom(&self, viewport: Rect) -> f32 {
        let avail = viewport.size() - Vec2::splat(2.0 * MARGIN);
        let z = (avail.x / self.doc.width() as f32).min(avail.y / self.doc.height() as f32);
        z.clamp(MIN_ZOOM, 1.0)
    }

    /// Changes the zoom, keeping `anchor` (a document point) fixed on screen.
    /// Without an anchor, the centre of the viewport stays put.
    pub fn zoom_to(&mut self, zoom: f32, anchor: Option<Point>, viewport: Rect) {
        self.view.auto_fit = false;
        let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let origin = self.canvas_origin(viewport);
        let anchor = anchor.unwrap_or_else(|| self.to_doc(viewport.center(), origin));
        let screen = self.to_screen(anchor, origin);
        self.view.zoom = zoom;
        let new_origin = screen - vec2(anchor.x, anchor.y) * zoom;
        self.view.scroll = (viewport.min + Vec2::splat(MARGIN)) - new_origin;
        self.clamp_scroll(viewport);
    }

    pub(crate) fn to_doc(&self, pos: Pos2, origin: Pos2) -> Point {
        let d = (pos - origin) / self.view.zoom;
        Point::from_xy(d.x, d.y)
    }

    pub(crate) fn to_screen(&self, p: Point, origin: Pos2) -> Pos2 {
        origin + vec2(p.x, p.y) * self.view.zoom
    }

    fn screen_rect(&self, r: tiny_skia::Rect, origin: Pos2) -> Rect {
        Rect::from_min_max(
            self.to_screen(Point::from_xy(r.left(), r.top()), origin),
            self.to_screen(Point::from_xy(r.right(), r.bottom()), origin),
        )
    }

    fn handle_at(&self, pos: Pos2, r: tiny_skia::Rect, origin: Pos2) -> Option<Handle> {
        let screen = self.screen_rect(r, origin);
        Handle::BOX.into_iter().find(|h| {
            let p = h.position(tiny_skia::Rect::from_ltrb(screen.min.x, screen.min.y, screen.max.x, screen.max.y).unwrap_or(r));
            (pos - pos2(p.x, p.y)).length() <= HANDLE + 3.0
        })
    }

    /// The canvas-resize handles (right edge, bottom edge, bottom-right corner).
    fn canvas_handle_at(&self, pos: Pos2, origin: Pos2) -> Option<Handle> {
        let r = tiny_skia::Rect::from_xywh(0.0, 0.0, self.doc.width() as f32, self.doc.height() as f32)?;
        let screen = self.screen_rect(r, origin);
        let points = [
            (Handle::E, pos2(screen.max.x + HANDLE, screen.center().y)),
            (Handle::S, pos2(screen.center().x, screen.max.y + HANDLE)),
            (Handle::SE, pos2(screen.max.x + HANDLE, screen.max.y + HANDLE)),
        ];
        points.into_iter().find(|(_, p)| (pos - *p).length() <= HANDLE + 3.0).map(|(h, _)| h)
    }

    // ------------------------------------------------------------------------------ frame

    pub(crate) fn workspace(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let p = Palette::of(&ctx);
        let full = ui.available_rect_before_wrap();
        ui.painter().rect_filled(full, 0.0, if self.view.fullscreen { Color32::from_gray(16) } else { p.workspace });
        let ruler = if self.view.show_rulers && !self.view.fullscreen { RULER } else { 0.0 };
        let viewport = Rect::from_min_max(full.min + vec2(ruler, ruler), full.max);
        self.viewport = viewport;
        let refit = self.view.auto_fit && (viewport.size() - self.view.fitted_for).length() > 0.5;
        if (self.view.fit_pending || refit) && viewport.width() > 50.0 && viewport.height() > 50.0 {
            self.view.fit_pending = false;
            self.view.auto_fit = true;
            self.view.fitted_for = viewport.size();
            self.view.zoom = self.fit_zoom(viewport);
            self.view.scroll = Vec2::ZERO;
        }
        let response = ui.interact(viewport, Id::unique("canvas"), Sense::click_and_drag());
        self.handle_wheel(ui, &response, viewport);
        self.clamp_scroll(viewport);
        let origin = self.canvas_origin(viewport);

        self.refresh_shape();
        let (w, h) = (self.doc.width(), self.doc.height());
        let mut canvas = std::mem::take(&mut self.canvas);
        canvas.sync(&ctx, w, h, |region| self.compose(region));
        self.canvas = canvas;

        let painter = ui.painter_at(viewport);
        let canvas_rect = Rect::from_min_size(origin, vec2(w as f32, h as f32) * self.view.zoom);
        if !self.view.fullscreen {
            painter.add(egui::Shadow {
                offset: [0, 2],
                blur: 12,
                spread: 0,
                color: p.canvas_shadow,
            }
            .as_shape(canvas_rect, 0));
        }
        let visible = canvas_rect.intersect(viewport);
        if visible.is_positive() {
            widgets::checkerboard(&painter, visible, 8.0);
        }
        self.canvas.paint(&painter, origin, self.view.zoom);

        self.handle_pointer(ui, &response, viewport, origin);
        self.draw_overlays(ui, viewport, origin);
        if ruler > 0.0 {
            self.draw_rulers(ui, full, viewport, origin);
        }
        self.draw_scrollbars(ui, viewport);
        if self.view.fullscreen && ui.input(|i| i.pointer.any_click()) && response.clicked() {
            self.execute(&ctx, super::Command::ToggleFullscreen);
        }
    }

    fn handle_wheel(&mut self, ui: &Ui, response: &egui::Response, viewport: Rect) {
        if !response.contains_pointer() {
            return;
        }
        let (zoom_delta, scroll, pointer) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.pointer.hover_pos()));
        if (zoom_delta - 1.0).abs() > 1e-4 {
            let origin = self.canvas_origin(viewport);
            let anchor = pointer.map(|p| self.to_doc(p, origin));
            self.zoom_to(self.view.zoom * zoom_delta, anchor, viewport);
        } else if scroll != Vec2::ZERO {
            self.view.scroll -= scroll;
        }
    }

    // ------------------------------------------------------------------------------ input

    fn handle_pointer(&mut self, ui: &Ui, response: &egui::Response, viewport: Rect, origin: Pos2) {
        let ctx = ui.ctx().clone();
        let (hover, interact, modifiers, time, dt, double, events) = ui.input(|i| {
            (
                i.pointer.hover_pos(),
                i.pointer.interact_pos(),
                i.modifiers,
                i.time,
                i.stable_dt,
                i.pointer.button_double_clicked(PointerButton::Primary),
                i.events.clone(),
            )
        });
        let over_canvas = response.contains_pointer() && hover.is_some_and(|p| viewport.contains(p));
        self.cursor_pixel = hover.filter(|_| over_canvas).map(|p| {
            let d = self.to_doc(p, origin);
            (d.x.floor() as i32, d.y.floor() as i32)
        });

        // Presses and releases, in order, so quick clicks aren't lost.
        for event in &events {
            match event {
                egui::Event::PointerButton {
                    pos,
                    button,
                    pressed: true,
                    ..
                } if self.gesture.is_none() && over_canvas && viewport.contains(*pos) && !self.view.fullscreen => {
                    let space = ui.input(|i| i.key_down(egui::Key::Space));
                    self.press(*button, *pos, origin, viewport, modifiers, space);
                }
                egui::Event::PointerMoved(pos) if !self.gesture.is_none() => {
                    self.drag(*pos, origin, modifiers, 0.0);
                }
                egui::Event::PointerButton { pos, pressed: false, .. } if !self.gesture.is_none() => {
                    self.drag(*pos, origin, modifiers, 0.0);
                    self.release(*pos, origin, double);
                }
                _ => {}
            }
        }
        if double && self.gesture.is_none() {
            if let Some(shape) = &self.shape {
                if shape.stage == ShapeStage::PolygonOpen {
                    self.close_polygon();
                }
            }
        }
        // Airbrush keeps spraying while held still.
        if let Gesture::Stroke { stroke, .. } = &self.gesture {
            if stroke.is_continuous() {
                if let Some(pos) = interact {
                    self.drag(pos, origin, modifiers, dt);
                }
                ctx.request_repaint();
            }
        }
        // Released outside the window or lost the pointer.
        if !self.gesture.is_none() && !ui.input(|i| i.pointer.any_down()) {
            let pos = interact.or(hover).unwrap_or(viewport.center());
            self.release(pos, origin, false);
        }
        self.update_cursor(&ctx, hover.filter(|_| over_canvas), origin, time);
    }

    fn press(&mut self, button: PointerButton, pos: Pos2, origin: Pos2, viewport: Rect, modifiers: egui::Modifiers, space: bool) {
        if button == PointerButton::Middle || (button == PointerButton::Primary && space) {
            self.gesture = Gesture::Pan { last: pos };
            return;
        }
        if !matches!(button, PointerButton::Primary | PointerButton::Secondary) {
            return;
        }
        let at = self.to_doc(pos, origin);
        if button == PointerButton::Primary {
            if let Some(handle) = self.canvas_handle_at(pos, origin) {
                self.commit_all();
                self.gesture = Gesture::ResizeCanvas {
                    handle,
                    width: self.doc.width(),
                    height: self.doc.height(),
                };
                return;
            }
        }
        match self.tool {
            Tool::Pencil | Tool::Brush | Tool::Eraser => self.begin_stroke(button, at),
            Tool::Fill => self.fill_at(button, at),
            Tool::ColorPicker => self.pick_at(button, at),
            Tool::Magnifier => self.magnify_at(button, at, viewport),
            Tool::Select => self.press_select(button, pos, at, origin, modifiers),
            Tool::Shape => self.press_shape(button, pos, at, origin),
            Tool::Text => self.press_text(button, pos, at, origin),
        }
    }

    fn press_select(&mut self, button: PointerButton, pos: Pos2, at: Point, origin: Pos2, modifiers: egui::Modifiers) {
        if let Some(active) = &self.selection {
            let bounds = active.sel.bounds();
            if button == PointerButton::Primary {
                if let Some(handle) = self.handle_at(pos, bounds, origin) {
                    self.lift_selection(false);
                    self.gesture = Gesture::ResizeSelection { handle, start: bounds, grab: at };
                    return;
                }
            }
            if active.sel.contains(at.x, at.y) {
                if modifiers.command && !active.sel.is_floating() {
                    self.lift_selection(true);
                } else if modifiers.command {
                    // Ctrl-drag a floating selection: stamp a copy and keep moving.
                    let key = self.transparent_selection.then_some(self.color2);
                    if let Some(active) = self.selection.as_ref() {
                        if let Some(index) = self.doc.layer_index(active.layer) {
                            let sel = active.sel.clone();
                            sel.draw_floating(&mut self.doc.layers[index].pixels, (0, 0), key);
                        }
                    }
                }
                self.gesture = Gesture::MoveSelection { last: at, moved: (0.0, 0.0) };
                return;
            }
        }
        if button == PointerButton::Primary {
            self.begin_marquee(at);
        } else {
            self.commit_selection();
        }
    }

    fn press_shape(&mut self, button: PointerButton, pos: Pos2, at: Point, origin: Pos2) {
        if let Some(shape) = &mut self.shape {
            match shape.stage {
                ShapeStage::CurveBend1 | ShapeStage::CurveBend2 => {
                    if let Geometry::Curve { c1, c2, .. } = &mut shape.geometry {
                        if shape.stage == ShapeStage::CurveBend1 {
                            *c1 = at;
                        }
                        *c2 = at;
                    }
                    self.gesture = Gesture::BendCurve;
                    return;
                }
                ShapeStage::PolygonOpen => {
                    if let Geometry::Polygon { points, .. } = &mut shape.geometry {
                        points.push(at);
                    }
                    self.gesture = Gesture::PlacePolygonPoint;
                    return;
                }
                ShapeStage::Editing => {
                    let geometry = shape.geometry.clone();
                    if let Geometry::Line { a, b } = geometry {
                        let near = |p: Point| (self.to_screen(p, origin) - pos).length() <= HANDLE + 3.0;
                        if near(a) || near(b) {
                            let handle = if near(a) { Handle::Start } else { Handle::End };
                            self.gesture = Gesture::ResizeShape { handle, start: geometry, grab: at };
                            return;
                        }
                    } else if let Some(bounds) = geometry.bounds() {
                        if let Some(handle) = self.handle_at(pos, bounds, origin) {
                            self.gesture = Gesture::ResizeShape { handle, start: geometry, grab: at };
                            return;
                        }
                    }
                    if let Some(b) = geometry.bounds() {
                        let pad = 4.0 / self.view.zoom;
                        if at.x >= b.left() - pad && at.x <= b.right() + pad && at.y >= b.top() - pad && at.y <= b.bottom() + pad {
                            self.gesture = Gesture::MoveShape { last: at };
                            return;
                        }
                    }
                    self.commit_shape();
                }
            }
        }
        self.begin_shape(button, at);
    }

    fn press_text(&mut self, button: PointerButton, pos: Pos2, at: Point, origin: Pos2) {
        if let Some(tb) = &self.text {
            let rect = tb.rect;
            if let Some(handle) = self.handle_at(pos, rect, origin) {
                self.gesture = Gesture::ResizeText { handle, start: rect, grab: at };
                return;
            }
            let pad = 6.0 / self.view.zoom;
            let inside = at.x >= rect.left() && at.x <= rect.right() && at.y >= rect.top() && at.y <= rect.bottom();
            let near_edge = at.x >= rect.left() - pad && at.x <= rect.right() + pad && at.y >= rect.top() - pad && at.y <= rect.bottom() + pad;
            if inside {
                if let Some(index) = self.text_hit(at) {
                    let shift = false;
                    if let Some(tb) = self.text.as_mut() {
                        tb.cursor = index;
                        if !shift {
                            tb.anchor = index;
                        }
                    }
                }
                self.gesture = Gesture::SelectText;
                return;
            }
            if near_edge {
                self.gesture = Gesture::MoveText { last: at };
                return;
            }
            self.commit_text();
            return;
        }
        let _ = button;
        self.begin_text_box(at);
    }

    fn drag(&mut self, pos: Pos2, origin: Pos2, modifiers: egui::Modifiers, dt: f32) {
        let at = self.to_doc(pos, origin);
        let shift = modifiers.shift;
        match &mut self.gesture {
            Gesture::None => {}
            Gesture::Stroke { .. } => self.continue_stroke(at, dt),
            Gesture::Pan { last } => {
                let delta = pos - *last;
                *last = pos;
                self.view.scroll -= delta;
            }
            Gesture::Marquee { start, end } => {
                *end = if shift { super::tools::constrain_square(*start, at) } else { at };
            }
            Gesture::Lasso { points } => {
                if points.last().is_none_or(|p| (p.x - at.x).abs() + (p.y - at.y).abs() >= 1.0) {
                    points.push(at);
                }
            }
            Gesture::MoveSelection { last, moved } => {
                moved.0 += at.x - last.x;
                moved.1 += at.y - last.y;
                *last = at;
                // Selections move in whole pixels.
                let (dx, dy) = (moved.0.trunc(), moved.1.trunc());
                moved.0 -= dx;
                moved.1 -= dy;
                if dx != 0.0 || dy != 0.0 {
                    self.move_selection(dx, dy);
                }
            }
            Gesture::ResizeSelection { handle, start, grab } => {
                let (dx, dy) = ((at.x - grab.x).round(), (at.y - grab.y).round());
                if let Some(r) = handle.drag(*start, dx, dy) {
                    self.resize_selection_to(r);
                }
            }
            Gesture::CreateShape { start } => {
                let start = *start;
                self.drag_new_shape(start, at, shift);
            }
            Gesture::MoveShape { last } => {
                let (dx, dy) = (at.x - last.x, at.y - last.y);
                *last = at;
                if let Some(shape) = self.shape.as_mut() {
                    shape.geometry.translate(dx, dy);
                }
            }
            Gesture::ResizeShape { handle, start, grab } => {
                let (dx, dy) = (at.x - grab.x, at.y - grab.y);
                let mut g = start.clone();
                match (&mut g, *handle) {
                    (Geometry::Line { a, .. }, Handle::Start) => *a = Point::from_xy(a.x + dx, a.y + dy),
                    (Geometry::Line { b, .. }, Handle::End) => *b = Point::from_xy(b.x + dx, b.y + dy),
                    (Geometry::Boxed { a, b, .. }, h) => {
                        if let Some(r) = tiny_skia::Rect::from_ltrb(a.x, a.y, b.x, b.y).and_then(|r| h.drag(r, dx, dy)) {
                            *a = Point::from_xy(r.left(), r.top());
                            *b = Point::from_xy(r.right(), r.bottom());
                        }
                    }
                    _ => {}
                }
                if let Some(shape) = self.shape.as_mut() {
                    shape.geometry = g;
                }
            }
            Gesture::BendCurve => {
                if let Some(shape) = self.shape.as_mut() {
                    let first = shape.stage == ShapeStage::CurveBend1;
                    if let Geometry::Curve { c1, c2, .. } = &mut shape.geometry {
                        if first {
                            *c1 = at;
                        }
                        *c2 = at;
                    }
                }
            }
            Gesture::PlacePolygonPoint => {
                if let Some(shape) = self.shape.as_mut() {
                    if let Geometry::Polygon { points, .. } = &mut shape.geometry {
                        let n = points.len();
                        let prev = if n >= 2 { points[n - 2] } else { at };
                        if let Some(last) = points.last_mut() {
                            *last = if shift { super::tools::constrain_angle(prev, at) } else { at };
                        }
                    }
                }
            }
            Gesture::CreateText { end, .. } => *end = at,
            Gesture::MoveText { last } => {
                let (dx, dy) = ((at.x - last.x).round(), (at.y - last.y).round());
                if dx != 0.0 || dy != 0.0 {
                    *last = Point::from_xy(last.x + dx, last.y + dy);
                    if let Some(tb) = self.text.as_mut() {
                        if let Some(r) = tiny_skia::Rect::from_xywh(tb.rect.x() + dx, tb.rect.y() + dy, tb.rect.width(), tb.rect.height()) {
                            tb.rect = r;
                        }
                    }
                    self.refresh_text(false);
                }
            }
            Gesture::ResizeText { handle, start, grab } => {
                let (dx, dy) = ((at.x - grab.x).round(), (at.y - grab.y).round());
                if let Some(r) = handle.drag(*start, dx, dy) {
                    if let Some(tb) = self.text.as_mut() {
                        tb.rect = r;
                    }
                    self.refresh_text(false);
                }
            }
            Gesture::SelectText => {
                if let Some(index) = self.text_hit(at) {
                    if let Some(tb) = self.text.as_mut() {
                        tb.cursor = index;
                    }
                }
            }
            Gesture::ResizeCanvas { handle, width, height } => {
                let max = crate::engine::document::MAX_DIMENSION as f32;
                if matches!(handle, Handle::E | Handle::SE) {
                    *width = at.x.round().clamp(1.0, max) as u32;
                }
                if matches!(handle, Handle::S | Handle::SE) {
                    *height = at.y.round().clamp(1.0, max) as u32;
                }
            }
        }
    }

    fn release(&mut self, pos: Pos2, origin: Pos2, double: bool) {
        let at = self.to_doc(pos, origin);
        match std::mem::replace(&mut self.gesture, Gesture::None) {
            Gesture::Stroke { stroke, area } => {
                self.gesture = Gesture::Stroke { stroke, area };
                self.end_stroke();
            }
            g @ (Gesture::Marquee { .. } | Gesture::Lasso { .. }) => {
                self.gesture = g;
                self.finish_marquee();
            }
            Gesture::CreateShape { .. } => self.finish_new_shape(),
            Gesture::BendCurve => {
                if let Some(shape) = self.shape.as_mut() {
                    if shape.stage == ShapeStage::CurveBend1 {
                        shape.stage = ShapeStage::CurveBend2;
                    } else {
                        self.commit_shape();
                    }
                }
            }
            Gesture::PlacePolygonPoint => {
                let close = double
                    || self.shape.as_ref().is_some_and(|s| match &s.geometry {
                        Geometry::Polygon { points, .. } if points.len() > 3 => {
                            let first = points[0];
                            (self.to_screen(first, origin) - self.to_screen(at, origin)).length() <= 8.0
                        }
                        _ => false,
                    });
                if close {
                    self.close_polygon();
                }
            }
            Gesture::CreateText { start, end } => {
                let swap = false;
                self.finish_text_box(start, end, swap);
            }
            Gesture::ResizeCanvas { width, height, .. } => {
                if (width, height) != (self.doc.width(), self.doc.height()) {
                    self.resize_canvas(width, height);
                }
            }
            _ => {}
        }
    }

    fn close_polygon(&mut self) {
        if let Some(shape) = self.shape.as_mut() {
            if let Geometry::Polygon { points, closed } = &mut shape.geometry {
                // A double-click adds a duplicate final point; drop it.
                while points.len() > 2 {
                    let n = points.len();
                    let (a, b) = (points[n - 1], points[n - 2]);
                    if (a.x - b.x).abs() < 1.0 && (a.y - b.y).abs() < 1.0 {
                        points.pop();
                    } else {
                        break;
                    }
                }
                if points.len() > 3 {
                    let (first, last) = (points[0], points[points.len() - 1]);
                    if (first.x - last.x).abs() < 3.0 && (first.y - last.y).abs() < 3.0 {
                        points.pop();
                    }
                }
                *closed = true;
            }
        }
        self.commit_shape();
    }

    fn update_cursor(&self, ctx: &egui::Context, hover: Option<Pos2>, origin: Pos2, _time: f64) {
        let Some(pos) = hover else {
            return;
        };
        let at = self.to_doc(pos, origin);
        let icon = match &self.gesture {
            Gesture::Pan { .. } => CursorIcon::Grabbing,
            Gesture::MoveSelection { .. } | Gesture::MoveShape { .. } | Gesture::MoveText { .. } => CursorIcon::Move,
            Gesture::ResizeSelection { handle, .. } | Gesture::ResizeShape { handle, .. } | Gesture::ResizeText { handle, .. } | Gesture::ResizeCanvas { handle, .. } => {
                handle.cursor()
            }
            _ => {
                if let Some(h) = self.canvas_handle_at(pos, origin) {
                    h.cursor()
                } else {
                    match self.tool {
                        Tool::Select => match &self.selection {
                            Some(s) if self.handle_at(pos, s.sel.bounds(), origin).is_some() => {
                                self.handle_at(pos, s.sel.bounds(), origin).map_or(CursorIcon::Default, |h| h.cursor())
                            }
                            Some(s) if s.sel.contains(at.x, at.y) => CursorIcon::Move,
                            _ => CursorIcon::Crosshair,
                        },
                        Tool::Text => match &self.text {
                            Some(tb) if self.handle_at(pos, tb.rect, origin).is_some() => {
                                self.handle_at(pos, tb.rect, origin).map_or(CursorIcon::Text, |h| h.cursor())
                            }
                            Some(tb) if at.x >= tb.rect.left() && at.x <= tb.rect.right() && at.y >= tb.rect.top() && at.y <= tb.rect.bottom() => {
                                CursorIcon::Text
                            }
                            Some(_) => CursorIcon::Move,
                            None => CursorIcon::Text,
                        },
                        Tool::Shape => match &self.shape {
                            Some(s) if s.stage == ShapeStage::Editing => match s.geometry.bounds() {
                                Some(b) if self.handle_at(pos, b, origin).is_some() => {
                                    self.handle_at(pos, b, origin).map_or(CursorIcon::Crosshair, |h| h.cursor())
                                }
                                Some(b) if at.x >= b.left() && at.x <= b.right() && at.y >= b.top() && at.y <= b.bottom() => CursorIcon::Move,
                                _ => CursorIcon::Crosshair,
                            },
                            _ => CursorIcon::Crosshair,
                        },
                        Tool::Magnifier => CursorIcon::ZoomIn,
                        Tool::Fill | Tool::ColorPicker => CursorIcon::Default,
                        Tool::Pencil | Tool::Brush | Tool::Eraser => {
                            if self.stroke_size() * self.view.zoom >= 6.0 {
                                CursorIcon::None
                            } else {
                                CursorIcon::Crosshair
                            }
                        }
                    }
                }
            }
        };
        ctx.set_cursor_icon(icon);
    }

    // ------------------------------------------------------------------------------ overlays

    fn draw_handles(&self, painter: &egui::Painter, r: Rect) {
        let p = Palette::of(painter.ctx());
        let rect = tiny_skia::Rect::from_ltrb(r.min.x, r.min.y, r.max.x, r.max.y);
        let Some(rect) = rect else {
            return;
        };
        for h in Handle::BOX {
            let c = h.position(rect);
            painter.rect(
                Rect::from_center_size(pos2(c.x, c.y), Vec2::splat(HANDLE)),
                CornerRadius::same(1),
                p.handle_fill,
                Stroke::new(1.0, p.handle_stroke),
                StrokeKind::Middle,
            );
        }
    }

    fn dashed_rect(painter: &egui::Painter, r: Rect, time: f64) {
        let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
        let offset = ((time * 12.0) % 8.0) as f32;
        painter.add(Shape::line(pts.to_vec(), Stroke::new(1.0, Color32::WHITE)));
        painter.extend(Shape::dashed_line_with_offset(&pts, Stroke::new(1.0, Color32::from_rgb(40, 40, 40)), &[4.0], &[4.0], offset));
    }

    fn draw_overlays(&mut self, ui: &Ui, viewport: Rect, origin: Pos2) {
        let painter = ui.painter_at(viewport);
        let p = Palette::of(ui.ctx());
        let time = ui.input(|i| i.time);
        let zoom = self.view.zoom;
        let canvas_rect = Rect::from_min_size(origin, vec2(self.doc.width() as f32, self.doc.height() as f32) * zoom);

        if self.view.show_grid {
            self.draw_grid(&painter, canvas_rect.intersect(viewport), origin);
        }

        // Selection.
        match &self.gesture {
            Gesture::Marquee { start, end } => {
                let r = Rect::from_two_pos(self.to_screen(*start, origin), self.to_screen(*end, origin));
                Self::dashed_rect(&painter, r, 0.0);
            }
            Gesture::Lasso { points } if points.len() > 1 => {
                let pts: Vec<Pos2> = points.iter().map(|p| self.to_screen(*p, origin)).collect();
                painter.add(Shape::line(pts.clone(), Stroke::new(1.0, Color32::WHITE)));
                painter.extend(Shape::dashed_line(&pts, Stroke::new(1.0, Color32::from_gray(40)), 4.0, 4.0));
            }
            _ => {}
        }
        if let Some(active) = &self.selection {
            let r = self.screen_rect(active.sel.bounds(), origin);
            if self.select_mode == SelectMode::FreeForm && active.sel.mask.is_some() && !active.sel.is_floating() {
                Self::dashed_rect(&painter, r, time);
            } else {
                Self::dashed_rect(&painter, r, time);
            }
            self.draw_handles(&painter, r);
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(120));
        }

        // Shape being edited.
        if let Some(shape) = &self.shape {
            if shape.stage == ShapeStage::Editing && matches!(self.gesture, Gesture::None | Gesture::MoveShape { .. } | Gesture::ResizeShape { .. }) {
                match &shape.geometry {
                    Geometry::Line { a, b } => {
                        for c in [a, b] {
                            let s = self.to_screen(*c, origin);
                            painter.circle(s, HANDLE / 2.0 + 1.0, p.handle_fill, Stroke::new(1.0, p.handle_stroke));
                        }
                    }
                    g => {
                        if let Some(b) = g.bounds() {
                            let r = self.screen_rect(b, origin);
                            Self::dashed_rect(&painter, r, 0.0);
                            self.draw_handles(&painter, r);
                        }
                    }
                }
            }
        }

        // Text box.
        if let Some(tb) = &self.text {
            let r = self.screen_rect(tb.rect, origin);
            Self::dashed_rect(&painter, r, 0.0);
            self.draw_handles(&painter, r);
            let style = self.current_text_style(tb.swap_colors);
            let (text, rect, cursor, (s0, s1)) = (tb.text.clone(), tb.rect, tb.cursor, tb.selection());
            let engine = self.fonts.get();
            let highlights = if s0 != s1 { engine.selection_rects(&text, &style, rect.width(), s0, s1) } else { Vec::new() };
            let (cx, cy) = engine.caret(&text, &style, rect.width(), cursor);
            let line = engine.line_height(&style);
            for hr in highlights {
                let min = self.to_screen(Point::from_xy(rect.x() + hr.x(), rect.y() + hr.y()), origin);
                painter.rect_filled(
                    Rect::from_min_size(min, vec2(hr.width(), hr.height()) * zoom),
                    0.0,
                    Color32::from_rgba_unmultiplied(0, 120, 215, 90),
                );
            }
            if ((time * 2.0) as i64) % 2 == 0 {
                let top = self.to_screen(Point::from_xy(rect.x() + cx, rect.y() + cy), origin);
                let caret_color = if p.dark { Color32::WHITE } else { Color32::BLACK };
                painter.line_segment([top, top + vec2(0.0, line * zoom)], Stroke::new(1.5, caret_color));
            }
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
        }

        // Text box being dragged out.
        if let Gesture::CreateText { start, end } = &self.gesture {
            Self::dashed_rect(&painter, Rect::from_two_pos(self.to_screen(*start, origin), self.to_screen(*end, origin)), 0.0);
        }

        // Canvas resize handles and preview.
        if !self.view.fullscreen {
            let (w, h) = match &self.gesture {
                Gesture::ResizeCanvas { width, height, .. } => (*width, *height),
                _ => (self.doc.width(), self.doc.height()),
            };
            if matches!(self.gesture, Gesture::ResizeCanvas { .. }) {
                let r = Rect::from_min_size(origin, vec2(w as f32, h as f32) * zoom);
                Self::dashed_rect(&painter, r, 0.0);
            }
            for c in [
                pos2(canvas_rect.max.x + HANDLE, canvas_rect.center().y),
                pos2(canvas_rect.center().x, canvas_rect.max.y + HANDLE),
                pos2(canvas_rect.max.x + HANDLE, canvas_rect.max.y + HANDLE),
            ] {
                painter.rect(
                    Rect::from_center_size(c, Vec2::splat(HANDLE)),
                    CornerRadius::same(1),
                    p.handle_fill,
                    Stroke::new(1.0, p.handle_stroke),
                    StrokeKind::Middle,
                );
            }
        }

        // Brush size outline.
        if let (Some(kind), Some(hover)) = (self.stroke_kind(), ui.input(|i| i.pointer.hover_pos())) {
            let size = self.stroke_size() * zoom;
            if size >= 6.0 && viewport.contains(hover) && !matches!(self.gesture, Gesture::Pan { .. } | Gesture::ResizeCanvas { .. }) {
                let outline = Stroke::new(1.0, Color32::from_gray(30));
                let inner = Stroke::new(1.0, Color32::from_gray(235));
                if matches!(kind, crate::engine::brush::StrokeKind::Eraser | crate::engine::brush::StrokeKind::Pencil) {
                    let doc = self.to_doc(hover, origin);
                    let half = (self.stroke_size().round() as i32 - 1) / 2;
                    let min = Point::from_xy((doc.x.floor() as i32 - half) as f32, (doc.y.floor() as i32 - half) as f32);
                    let r = Rect::from_min_size(self.to_screen(min, origin), Vec2::splat(self.stroke_size().round() * zoom));
                    painter.rect_stroke(r, 0.0, outline, StrokeKind::Outside);
                    painter.rect_stroke(r, 0.0, inner, StrokeKind::Inside);
                } else {
                    painter.circle_stroke(hover, size / 2.0 + 0.5, outline);
                    painter.circle_stroke(hover, size / 2.0 - 0.5, inner);
                }
            }
        }
    }

    fn draw_grid(&self, painter: &egui::Painter, visible: Rect, origin: Pos2) {
        if !visible.is_positive() {
            return;
        }
        let zoom = self.view.zoom;
        let step = [1.0f32, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0]
            .into_iter()
            .find(|s| s * zoom >= 8.0)
            .unwrap_or(1000.0);
        let color = Color32::from_rgba_unmultiplied(128, 128, 128, 110);
        let mut shapes = Vec::new();
        let x0 = ((visible.min.x - origin.x) / zoom / step).floor() as i64;
        let x1 = ((visible.max.x - origin.x) / zoom / step).ceil() as i64;
        for i in x0..=x1 {
            let x = origin.x + i as f32 * step * zoom;
            if x >= visible.min.x && x <= visible.max.x {
                shapes.push(Shape::vline(x, visible.y_range(), Stroke::new(1.0, color)));
            }
        }
        let y0 = ((visible.min.y - origin.y) / zoom / step).floor() as i64;
        let y1 = ((visible.max.y - origin.y) / zoom / step).ceil() as i64;
        for i in y0..=y1 {
            let y = origin.y + i as f32 * step * zoom;
            if y >= visible.min.y && y <= visible.max.y {
                shapes.push(Shape::hline(visible.x_range(), y, Stroke::new(1.0, color)));
            }
        }
        painter.extend(shapes);
    }

    fn draw_rulers(&self, ui: &Ui, full: Rect, viewport: Rect, origin: Pos2) {
        let painter = ui.painter_at(full);
        let p = Palette::of(ui.ctx());
        let top = Rect::from_min_max(pos2(viewport.min.x, full.min.y), pos2(full.max.x, viewport.min.y));
        let left = Rect::from_min_max(pos2(full.min.x, viewport.min.y), pos2(viewport.min.x, full.max.y));
        for r in [top, left, Rect::from_min_max(full.min, viewport.min)] {
            painter.rect_filled(r, 0.0, p.chrome);
        }
        painter.hline(top.x_range(), top.max.y - 0.5, Stroke::new(1.0, p.divider));
        painter.vline(left.max.x - 0.5, left.y_range(), Stroke::new(1.0, p.divider));
        let zoom = self.view.zoom;
        let step = [1.0f32, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0]
            .into_iter()
            .find(|s| s * zoom >= 60.0)
            .unwrap_or(10_000.0);
        let minor = step / 10.0;
        let text = p.text_secondary;
        let tick = Stroke::new(1.0, p.text_secondary.gamma_multiply(0.7));
        // Horizontal ruler.
        let start = ((top.min.x - origin.x) / zoom / minor).floor() as i64;
        let end = ((top.max.x - origin.x) / zoom / minor).ceil() as i64;
        for i in start..=end {
            let v = i as f32 * minor;
            let x = origin.x + v * zoom;
            if x < top.min.x || x > top.max.x {
                continue;
            }
            let major = (i % 10) == 0;
            let len = if major { RULER - 4.0 } else if i % 5 == 0 { 7.0 } else { 4.0 };
            if minor * zoom >= 3.0 || major {
                painter.vline(x, (top.max.y - len)..=top.max.y, tick);
            }
            if major {
                painter.text(pos2(x + 3.0, top.min.y + 2.0), Align2::LEFT_TOP, format!("{}", v as i64), egui::FontId::proportional(10.0), text);
            }
        }
        // Vertical ruler.
        let start = ((left.min.y - origin.y) / zoom / minor).floor() as i64;
        let end = ((left.max.y - origin.y) / zoom / minor).ceil() as i64;
        for i in start..=end {
            let v = i as f32 * minor;
            let y = origin.y + v * zoom;
            if y < left.min.y || y > left.max.y {
                continue;
            }
            let major = (i % 10) == 0;
            let len = if major { RULER - 4.0 } else if i % 5 == 0 { 7.0 } else { 4.0 };
            if minor * zoom >= 3.0 || major {
                painter.hline((left.max.x - len)..=left.max.x, y, tick);
            }
            if major {
                let galley = painter.layout_no_wrap(format!("{}", v as i64), egui::FontId::proportional(10.0), text);
                let pos = pos2(left.min.x + 2.0, y + 3.0);
                painter.add(egui::epaint::TextShape::new(pos + vec2(galley.size().y, 0.0), galley, text).with_angle(std::f32::consts::FRAC_PI_2));
            }
        }
        // Pointer position markers.
        if let Some((x, y)) = self.cursor_pixel {
            let sx = origin.x + (x as f32 + 0.5) * zoom;
            let sy = origin.y + (y as f32 + 0.5) * zoom;
            if top.x_range().contains(sx) {
                painter.vline(sx, top.y_range(), Stroke::new(1.0, p.accent));
            }
            if left.y_range().contains(sy) {
                painter.hline(left.x_range(), sy, Stroke::new(1.0, p.accent));
            }
        }
    }

    fn draw_scrollbars(&mut self, ui: &Ui, viewport: Rect) {
        let max = self.max_scroll(viewport);
        let p = Palette::of(ui.ctx());
        let content = vec2(self.doc.width() as f32, self.doc.height() as f32) * self.view.zoom + Vec2::splat(2.0 * MARGIN);
        for horizontal in [true, false] {
            let (max_s, len, total) = if horizontal {
                (max.x, viewport.width(), content.x)
            } else {
                (max.y, viewport.height(), content.y)
            };
            if max_s <= 0.0 {
                continue;
            }
            let track = if horizontal {
                Rect::from_min_max(pos2(viewport.min.x + 2.0, viewport.max.y - SCROLLBAR - 2.0), pos2(viewport.max.x - SCROLLBAR - 4.0, viewport.max.y - 2.0))
            } else {
                Rect::from_min_max(pos2(viewport.max.x - SCROLLBAR - 2.0, viewport.min.y + 2.0), pos2(viewport.max.x - 2.0, viewport.max.y - SCROLLBAR - 4.0))
            };
            let track_len = if horizontal { track.width() } else { track.height() };
            let thumb_len = (len / total * track_len).max(24.0);
            let scroll = if horizontal { self.view.scroll.x } else { self.view.scroll.y };
            let offset = scroll / max_s * (track_len - thumb_len);
            let thumb = if horizontal {
                Rect::from_min_size(pos2(track.min.x + offset, track.min.y), vec2(thumb_len, track.height()))
            } else {
                Rect::from_min_size(pos2(track.min.x, track.min.y + offset), vec2(track.width(), thumb_len))
            };
            let id = Id::unique(("scrollbar", horizontal));
            let response = ui.interact(thumb.expand(2.0), id, Sense::drag());
            if response.dragged() {
                let d = response.drag_delta();
                let delta = if horizontal { d.x } else { d.y } * (max_s / (track_len - thumb_len).max(1.0));
                if horizontal {
                    self.view.scroll.x += delta;
                } else {
                    self.view.scroll.y += delta;
                }
            }
            let color = if response.hovered() || response.dragged() { p.text_secondary } else { p.text_secondary.gamma_multiply(0.55) };
            ui.painter().rect_filled(thumb, CornerRadius::same(4), color);
        }
    }

    /// Floating "Thumbnail" window showing the whole picture.
    pub(crate) fn thumbnail_window(&mut self, ctx: &egui::Context) {
        if !self.view.show_thumbnail {
            return;
        }
        let mut open = true;
        egui::Window::new("Thumbnail")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_pos(pos2(ctx.content_rect().max.x - 260.0, 160.0))
            .show(ctx, |ui| {
                let max = vec2(220.0, 160.0);
                let scale = (max.x / self.doc.width() as f32).min(max.y / self.doc.height() as f32);
                let size = vec2(self.doc.width() as f32, self.doc.height() as f32) * scale;
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                widgets::checkerboard(ui.painter(), rect, 6.0);
                self.canvas.paint(&ui.painter_at(rect), rect.min, scale);
            });
        if !open {
            self.view.show_thumbnail = false;
        }
    }
}
