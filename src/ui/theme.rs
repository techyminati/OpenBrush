// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Windows 11 (Fluent) colours, fonts and egui style.

use std::sync::Arc;

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, Visuals};

/// The user's theme preference.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ThemePref {
    Light,
    Dark,
    #[default]
    System,
}

impl ThemePref {
    pub fn label(self) -> &'static str {
        match self {
            ThemePref::Light => "Light",
            ThemePref::Dark => "Dark",
            ThemePref::System => "Use system setting",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            ThemePref::Light => "light",
            ThemePref::Dark => "dark",
            ThemePref::System => "system",
        }
    }

    pub fn from_key(key: &str) -> Self {
        match key {
            "light" => ThemePref::Light,
            "dark" => ThemePref::Dark,
            _ => ThemePref::System,
        }
    }
}

/// Semantic colours for the current theme.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    /// Title, menu, toolbar and status bar background.
    pub chrome: Color32,
    /// Area around the canvas.
    pub workspace: Color32,
    /// Popups, menus and dialogs.
    pub surface: Color32,
    pub surface_alt: Color32,
    pub border: Color32,
    pub divider: Color32,
    pub text: Color32,
    pub text_secondary: Color32,
    pub text_disabled: Color32,
    pub icon: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub selected: Color32,
    pub accent: Color32,
    pub accent_text: Color32,
    pub input_bg: Color32,
    pub canvas_shadow: Color32,
    pub handle_fill: Color32,
    pub handle_stroke: Color32,
}

impl Palette {
    pub fn light() -> Self {
        Self {
            dark: false,
            chrome: Color32::from_rgb(0xF3, 0xF3, 0xF3),
            workspace: Color32::from_rgb(0xE6, 0xEA, 0xEF),
            surface: Color32::from_rgb(0xF9, 0xF9, 0xF9),
            surface_alt: Color32::from_rgb(0xF3, 0xF3, 0xF3),
            border: Color32::from_rgb(0xE0, 0xE0, 0xE0),
            divider: Color32::from_rgb(0xDC, 0xDC, 0xDC),
            text: Color32::from_rgb(0x1B, 0x1B, 0x1B),
            text_secondary: Color32::from_rgb(0x5D, 0x5D, 0x5D),
            text_disabled: Color32::from_rgb(0xA0, 0xA0, 0xA0),
            icon: Color32::from_rgb(0x24, 0x24, 0x24),
            hover: Color32::from_black_alpha(12),
            pressed: Color32::from_black_alpha(20),
            selected: Color32::from_rgb(0xD8, 0xE6, 0xF5),
            accent: Color32::from_rgb(0x00, 0x5F, 0xB8),
            accent_text: Color32::WHITE,
            input_bg: Color32::WHITE,
            canvas_shadow: Color32::from_black_alpha(28),
            handle_fill: Color32::WHITE,
            handle_stroke: Color32::from_rgb(0x5A, 0x5A, 0x5A),
        }
    }

    pub fn dark() -> Self {
        Self {
            dark: true,
            chrome: Color32::from_rgb(0x20, 0x20, 0x20),
            workspace: Color32::from_rgb(0x17, 0x17, 0x17),
            surface: Color32::from_rgb(0x2C, 0x2C, 0x2C),
            surface_alt: Color32::from_rgb(0x20, 0x20, 0x20),
            border: Color32::from_rgb(0x3F, 0x3F, 0x3F),
            divider: Color32::from_rgb(0x3A, 0x3A, 0x3A),
            text: Color32::WHITE,
            text_secondary: Color32::from_rgb(0xC8, 0xC8, 0xC8),
            text_disabled: Color32::from_rgb(0x78, 0x78, 0x78),
            icon: Color32::from_rgb(0xF0, 0xF0, 0xF0),
            hover: Color32::from_white_alpha(15),
            pressed: Color32::from_white_alpha(10),
            selected: Color32::from_rgb(0x3A, 0x45, 0x52),
            accent: Color32::from_rgb(0x60, 0xCD, 0xFF),
            accent_text: Color32::BLACK,
            input_bg: Color32::from_rgb(0x2D, 0x2D, 0x2D),
            canvas_shadow: Color32::from_black_alpha(90),
            handle_fill: Color32::from_rgb(0x2C, 0x2C, 0x2C),
            handle_stroke: Color32::from_rgb(0xD0, 0xD0, 0xD0),
        }
    }

    pub fn for_dark(dark: bool) -> Self {
        if dark { Self::dark() } else { Self::light() }
    }

    /// The palette egui is currently using.
    pub fn of(ctx: &egui::Context) -> Self {
        Self::for_dark(ctx.global_style().visuals.dark_mode)
    }
}

pub const SEMIBOLD: &str = "semibold";

pub fn body() -> FontId {
    FontId::proportional(14.0)
}

pub fn caption() -> FontId {
    FontId::proportional(12.0)
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

/// Uses Segoe UI Variable / Segoe UI when available (Windows), otherwise the bundled
/// Selawik, which is metric-compatible with Segoe UI.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let system = |paths: &[&str]| paths.iter().find_map(|p| std::fs::read(p).ok());
    let regular = system(&[r"C:\Windows\Fonts\SegUIVar.ttf", r"C:\Windows\Fonts\segoeui.ttf"])
        .unwrap_or_else(|| include_bytes!("../../assets/fonts/selawk.ttf").to_vec());
    let bold = system(&[r"C:\Windows\Fonts\seguisb.ttf"])
        .unwrap_or_else(|| include_bytes!("../../assets/fonts/selawksb.ttf").to_vec());
    fonts.font_data.insert("ui".into(), Arc::new(FontData::from_owned(regular)));
    fonts.font_data.insert("ui-semibold".into(), Arc::new(FontData::from_owned(bold)));
    let fallbacks = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mut proportional = vec!["ui".to_owned()];
    proportional.extend(fallbacks.iter().cloned());
    fonts.families.insert(FontFamily::Proportional, proportional);
    let mut semibold_family = vec!["ui-semibold".to_owned()];
    semibold_family.extend(fallbacks);
    fonts.families.insert(FontFamily::Name(SEMIBOLD.into()), semibold_family);
    ctx.set_fonts(fonts);
}

/// Configures egui's built-in widgets (text fields, sliders, scroll bars…) to look Fluent.
pub fn apply(ctx: &egui::Context, dark: bool) {
    let p = Palette::for_dark(dark);
    let mut visuals = if dark { Visuals::dark() } else { Visuals::light() };
    visuals.panel_fill = p.chrome;
    visuals.window_fill = p.surface;
    visuals.extreme_bg_color = p.input_bg;
    visuals.faint_bg_color = p.surface_alt;
    visuals.window_stroke = Stroke::new(1.0, p.border);
    visuals.window_corner_radius = CornerRadius::same(8);
    visuals.menu_corner_radius = CornerRadius::same(8);
    visuals.selection.bg_fill = p.accent.gamma_multiply(if dark { 0.5 } else { 0.35 });
    visuals.selection.stroke = Stroke::new(1.0, p.accent);
    visuals.hyperlink_color = p.accent;
    visuals.override_text_color = Some(p.text);
    visuals.window_shadow = egui::Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 110 } else { 40 }),
    };
    visuals.popup_shadow = egui::Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 100 } else { 30 }),
    };
    for w in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
        &mut visuals.widgets.noninteractive,
    ] {
        w.corner_radius = CornerRadius::same(4);
    }
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.divider);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.inactive.bg_fill = p.input_bg;
    visuals.widgets.inactive.weak_bg_fill = if dark { Color32::from_rgb(0x37, 0x37, 0x37) } else { Color32::WHITE };
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, p.border);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.hovered.bg_fill = p.input_bg;
    visuals.widgets.hovered.weak_bg_fill = if dark { Color32::from_rgb(0x3C, 0x3C, 0x3C) } else { Color32::from_rgb(0xF6, 0xF6, 0xF6) };
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, p.border);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.active.bg_fill = p.accent;
    visuals.widgets.active.weak_bg_fill = if dark { Color32::from_rgb(0x33, 0x33, 0x33) } else { Color32::from_rgb(0xF0, 0xF0, 0xF0) };
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, p.text);
    visuals.slider_trailing_fill = true;

    ctx.set_visuals_of(if dark { egui::Theme::Dark } else { egui::Theme::Light }, visuals);
    ctx.set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
    ctx.all_styles_mut(|style| {
        style.spacing.button_padding = egui::vec2(12.0, 5.0);
        style.spacing.interact_size = egui::vec2(32.0, 32.0);
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.slider_rail_height = 4.0;
        style.spacing.scroll = egui::style::ScrollStyle::thin();
        style.text_styles.insert(egui::TextStyle::Body, body());
        style.text_styles.insert(egui::TextStyle::Button, body());
        style.text_styles.insert(egui::TextStyle::Small, caption());
        style.text_styles.insert(egui::TextStyle::Heading, semibold(20.0));
        style.interaction.tooltip_delay = 0.6;
    });
}
