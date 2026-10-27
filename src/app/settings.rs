// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Remembering preferences between sessions.

use std::path::PathBuf;

use super::PaintApp;
use crate::engine::brush::BrushKind;
use crate::engine::color::Rgba;
use crate::engine::shapes::{PaintStyle, ShapeKind};
use crate::ui::theme::ThemePref;

fn get(storage: &dyn eframe::Storage, key: &str) -> Option<String> {
    storage.get_string(&format!("openbrush.{key}"))
}

fn set(storage: &mut dyn eframe::Storage, key: &str, value: String) {
    storage.set_string(&format!("openbrush.{key}"), value);
}

fn flag(storage: &dyn eframe::Storage, key: &str) -> Option<bool> {
    get(storage, key).map(|v| v == "1")
}

fn number(storage: &dyn eframe::Storage, key: &str) -> Option<u32> {
    get(storage, key).and_then(|v| v.parse().ok())
}

fn index_of<T: PartialEq + Copy>(all: &[T], value: T) -> String {
    all.iter().position(|v| *v == value).unwrap_or(0).to_string()
}

fn from_index<T: Copy>(all: &[T], text: Option<String>) -> Option<T> {
    text.and_then(|t| t.parse::<usize>().ok()).and_then(|i| all.get(i).copied())
}

impl PaintApp {
    pub(crate) fn load_settings(&mut self, storage: &dyn eframe::Storage) {
        if let Some(theme) = get(storage, "theme") {
            self.theme = ThemePref::from_key(&theme);
        }
        if let Some(c) = get(storage, "color1").and_then(|h| Rgba::from_hex(&h)) {
            self.color1 = c;
        }
        if let Some(c) = get(storage, "color2").and_then(|h| Rgba::from_hex(&h)) {
            self.color2 = c;
        }
        if let Some(list) = get(storage, "custom_colors") {
            self.custom_colors = list.split(',').filter_map(Rgba::from_hex).take(10).collect();
        }
        if let Some(list) = get(storage, "recent") {
            self.recent = list.lines().filter(|l| !l.is_empty()).map(PathBuf::from).filter(|p| p.exists()).collect();
        }
        if let Some(v) = flag(storage, "rulers") {
            self.view.show_rulers = v;
        }
        if let Some(v) = flag(storage, "grid") {
            self.view.show_grid = v;
        }
        if let Some(v) = flag(storage, "status_bar") {
            self.view.show_status_bar = v;
        }
        if let Some(v) = flag(storage, "transparent_selection") {
            self.transparent_selection = v;
        }
        for (key, kind) in [
            ("size.pencil", super::state::SizeKind::Pencil),
            ("size.brush", super::state::SizeKind::Brush),
            ("size.eraser", super::state::SizeKind::Eraser),
            ("size.shape", super::state::SizeKind::Shape),
        ] {
            if let Some(v) = number(storage, key) {
                self.sizes.set(kind, v);
            }
        }
        if let Some(b) = from_index(&BrushKind::ALL, get(storage, "brush")) {
            self.brush = b;
        }
        if let Some(s) = from_index(&ShapeKind::ALL, get(storage, "shape")) {
            self.shape_kind = s;
        }
        if let Some(s) = from_index(&PaintStyle::ALL, get(storage, "outline")) {
            self.outline = s;
        }
        if let Some(s) = from_index(&PaintStyle::ALL, get(storage, "fill")) {
            self.fill = s;
        }
        if let Some(f) = get(storage, "font") {
            self.text_style.family = f;
        }
        if let Some(s) = get(storage, "font_size").and_then(|s| s.parse::<f32>().ok()) {
            self.text_style.size_pt = s.clamp(1.0, 400.0);
        }
    }

    pub(crate) fn save_settings(&self, storage: &mut dyn eframe::Storage) {
        let b = |v: bool| if v { "1".to_owned() } else { "0".to_owned() };
        set(storage, "theme", self.theme.key().to_owned());
        set(storage, "color1", self.color1.to_hex());
        set(storage, "color2", self.color2.to_hex());
        set(storage, "custom_colors", self.custom_colors.iter().map(|c| c.to_hex()).collect::<Vec<_>>().join(","));
        set(storage, "recent", self.recent.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n"));
        set(storage, "rulers", b(self.view.show_rulers));
        set(storage, "grid", b(self.view.show_grid));
        set(storage, "status_bar", b(self.view.show_status_bar));
        set(storage, "transparent_selection", b(self.transparent_selection));
        set(storage, "size.pencil", self.sizes.pencil.to_string());
        set(storage, "size.brush", self.sizes.brush.to_string());
        set(storage, "size.eraser", self.sizes.eraser.to_string());
        set(storage, "size.shape", self.sizes.shape.to_string());
        set(storage, "brush", index_of(&BrushKind::ALL, self.brush));
        set(storage, "shape", index_of(&ShapeKind::ALL, self.shape_kind));
        set(storage, "outline", index_of(&PaintStyle::ALL, self.outline));
        set(storage, "fill", index_of(&PaintStyle::ALL, self.fill));
        set(storage, "font", self.text_style.family.clone());
        set(storage, "font_size", self.text_style.size_pt.to_string());
    }
}
