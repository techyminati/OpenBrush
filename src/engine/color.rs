// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Straight-alpha RGBA colours and conversions.

use tiny_skia::{ColorU8, PremultipliedColorU8};

/// A colour with straight (non-premultiplied) alpha.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const BLACK: Rgba = Rgba::rgb(0, 0, 0);
    pub const WHITE: Rgba = Rgba::rgb(255, 255, 255);
    pub const TRANSPARENT: Rgba = Rgba::new(0, 0, 0, 0);

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    pub fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    /// `#RRGGBB`, or `#RRGGBBAA` when the colour is not opaque.
    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
        }
    }

    /// Parses `RGB`, `RRGGBB` or `RRGGBBAA`, with or without a leading `#`.
    pub fn from_hex(text: &str) -> Option<Self> {
        let hex = text.trim().trim_start_matches('#');
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        match hex.len() {
            3 => {
                let nib = |i: usize| u8::from_str_radix(&hex[i..=i], 16).ok().map(|v| v * 17);
                Some(Self::rgb(nib(0)?, nib(1)?, nib(2)?))
            }
            6 => Some(Self::rgb(byte(0)?, byte(2)?, byte(4)?)),
            8 => Some(Self::new(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
            _ => None,
        }
    }

    pub fn to_skia(self) -> tiny_skia::Color {
        tiny_skia::Color::from_rgba8(self.r, self.g, self.b, self.a)
    }

    pub fn premultiply(self) -> PremultipliedColorU8 {
        ColorU8::from_rgba(self.r, self.g, self.b, self.a).premultiply()
    }

    pub fn from_premultiplied(p: PremultipliedColorU8) -> Self {
        let c = p.demultiply();
        Self::new(c.red(), c.green(), c.blue(), c.alpha())
    }

    /// Hue in degrees `[0, 360)`, saturation and value in `[0, 1]`.
    pub fn to_hsv(self) -> (f32, f32, f32) {
        let r = self.r as f32 / 255.0;
        let g = self.g as f32 / 255.0;
        let b = self.b as f32 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        let hue = if delta == 0.0 {
            0.0
        } else if max == r {
            60.0 * ((g - b) / delta).rem_euclid(6.0)
        } else if max == g {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };
        let saturation = if max == 0.0 { 0.0 } else { delta / max };
        (hue, saturation, max)
    }

    pub fn from_hsv(hue: f32, saturation: f32, value: f32) -> Self {
        let h = hue.rem_euclid(360.0) / 60.0;
        let s = saturation.clamp(0.0, 1.0);
        let v = value.clamp(0.0, 1.0);
        let c = v * s;
        let x = c * (1.0 - (h % 2.0 - 1.0).abs());
        let (r, g, b) = match h as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = v - c;
        let to_u8 = |f: f32| ((f + m) * 255.0).round().clamp(0.0, 255.0) as u8;
        Self::rgb(to_u8(r), to_u8(g), to_u8(b))
    }

    /// Largest per-channel difference, used as a fill tolerance metric.
    pub fn max_channel_diff(self, other: Rgba) -> u8 {
        let d = |a: u8, b: u8| a.abs_diff(b);
        d(self.r, other.r)
            .max(d(self.g, other.g))
            .max(d(self.b, other.b))
            .max(d(self.a, other.a))
    }
}

/// The default Windows 11 Paint palette: two rows of ten colours.
pub const DEFAULT_PALETTE: [Rgba; 20] = [
    Rgba::rgb(0, 0, 0),
    Rgba::rgb(127, 127, 127),
    Rgba::rgb(136, 0, 21),
    Rgba::rgb(237, 28, 36),
    Rgba::rgb(255, 127, 39),
    Rgba::rgb(255, 242, 0),
    Rgba::rgb(34, 177, 76),
    Rgba::rgb(0, 162, 232),
    Rgba::rgb(63, 72, 204),
    Rgba::rgb(163, 73, 164),
    Rgba::rgb(255, 255, 255),
    Rgba::rgb(195, 195, 195),
    Rgba::rgb(185, 122, 87),
    Rgba::rgb(255, 174, 201),
    Rgba::rgb(255, 201, 14),
    Rgba::rgb(239, 228, 176),
    Rgba::rgb(181, 230, 29),
    Rgba::rgb(153, 217, 234),
    Rgba::rgb(112, 146, 190),
    Rgba::rgb(200, 191, 231),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        let c = Rgba::rgb(0x12, 0xAB, 0xEF);
        assert_eq!(c.to_hex(), "#12ABEF");
        assert_eq!(Rgba::from_hex("#12abef"), Some(c));
        assert_eq!(Rgba::from_hex("12ABEF"), Some(c));
        assert_eq!(Rgba::from_hex("#fff"), Some(Rgba::WHITE));
        assert_eq!(Rgba::from_hex("#11223380"), Some(Rgba::new(0x11, 0x22, 0x33, 0x80)));
        assert_eq!(Rgba::new(1, 2, 3, 4).to_hex(), "#01020304");
    }

    #[test]
    fn hex_rejects_garbage() {
        assert_eq!(Rgba::from_hex(""), None);
        assert_eq!(Rgba::from_hex("#12345"), None);
        assert_eq!(Rgba::from_hex("#zzzzzz"), None);
        assert_eq!(Rgba::from_hex("#ÿÿÿ"), None);
    }

    #[test]
    fn hsv_round_trip_for_palette() {
        for c in DEFAULT_PALETTE {
            let (h, s, v) = c.to_hsv();
            assert_eq!(Rgba::from_hsv(h, s, v), c, "colour {}", c.to_hex());
        }
    }

    #[test]
    fn hsv_primaries() {
        assert_eq!(Rgba::from_hsv(0.0, 1.0, 1.0), Rgba::rgb(255, 0, 0));
        assert_eq!(Rgba::from_hsv(120.0, 1.0, 1.0), Rgba::rgb(0, 255, 0));
        assert_eq!(Rgba::from_hsv(240.0, 1.0, 1.0), Rgba::rgb(0, 0, 255));
        assert_eq!(Rgba::from_hsv(360.0, 1.0, 1.0), Rgba::rgb(255, 0, 0));
        assert_eq!(Rgba::rgb(0, 0, 255).to_hsv(), (240.0, 1.0, 1.0));
    }

    #[test]
    fn premultiply_round_trip_opaque() {
        let c = Rgba::rgb(10, 200, 30);
        assert_eq!(Rgba::from_premultiplied(c.premultiply()), c);
    }
}
