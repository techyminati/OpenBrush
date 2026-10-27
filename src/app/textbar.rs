// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! The Text formatting bar, and keyboard editing inside a text box.

use egui::{Event, Key, Ui, vec2};

use super::PaintApp;
use crate::engine::text::TextAlign;
use crate::ui::icons::Icon;
use crate::ui::widgets;

const SIZES: [f32; 16] = [8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0, 28.0, 36.0, 48.0, 72.0];

fn prev_boundary(text: &str, i: usize) -> usize {
    text[..i].char_indices().next_back().map_or(0, |(j, _)| j)
}

fn next_boundary(text: &str, i: usize) -> usize {
    text[i..].chars().next().map_or(text.len(), |c| i + c.len_utf8())
}

fn prev_word(text: &str, i: usize) -> usize {
    let mut j = i;
    while j > 0 && text[..j].ends_with(char::is_whitespace) {
        j = prev_boundary(text, j);
    }
    while j > 0 && !text[..j].ends_with(char::is_whitespace) {
        j = prev_boundary(text, j);
    }
    j
}

fn next_word(text: &str, i: usize) -> usize {
    let mut j = i;
    while j < text.len() && !text[j..].starts_with(char::is_whitespace) {
        j = next_boundary(text, j);
    }
    while j < text.len() && text[j..].starts_with(char::is_whitespace) {
        j = next_boundary(text, j);
    }
    j
}

fn line_start(text: &str, i: usize) -> usize {
    text[..i].rfind('\n').map_or(0, |j| j + 1)
}

fn line_end(text: &str, i: usize) -> usize {
    text[i..].find('\n').map_or(text.len(), |j| i + j)
}

impl PaintApp {
    pub(crate) fn text_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if self.text_style.family.is_empty() {
                self.text_style.family = self.fonts.get().default_family();
            }
            let families = self.fonts.get().families().to_vec();
            let mut family = self.text_style.family.clone();
            egui::ComboBox::from_id_salt("font-family")
                .width(200.0)
                .height(400.0)
                .selected_text(family.clone())
                .show_ui(ui, |ui| {
                    for f in &families {
                        ui.selectable_value(&mut family, f.clone(), f);
                    }
                });
            if family != self.text_style.family {
                self.text_style.family = family;
            }
            let mut size = self.text_style.size_pt;
            egui::ComboBox::from_id_salt("font-size")
                .width(64.0)
                .selected_text(format!("{size}"))
                .show_ui(ui, |ui| {
                    for s in SIZES {
                        ui.selectable_value(&mut size, s, format!("{s}"));
                    }
                });
            ui.add(egui::DragValue::new(&mut size).range(1.0..=400.0).speed(0.2).suffix(" pt"));
            self.text_style.size_pt = size;
            widgets::vertical_divider(ui, 30.0);
            let b = vec2(32.0, 30.0);
            let s = &mut self.text_style;
            if widgets::icon_button(ui, Icon::Bold, b, s.bold, true, "Bold").clicked() {
                s.bold = !s.bold;
            }
            if widgets::icon_button(ui, Icon::Italic, b, s.italic, true, "Italic").clicked() {
                s.italic = !s.italic;
            }
            if widgets::icon_button(ui, Icon::Underline, b, s.underline, true, "Underline").clicked() {
                s.underline = !s.underline;
            }
            if widgets::icon_button(ui, Icon::Strikethrough, b, s.strikethrough, true, "Strikethrough").clicked() {
                s.strikethrough = !s.strikethrough;
            }
            widgets::vertical_divider(ui, 30.0);
            for (align, icon, label) in [
                (TextAlign::Left, Icon::AlignLeft, "Align left"),
                (TextAlign::Center, Icon::AlignCenter, "Center"),
                (TextAlign::Right, Icon::AlignRight, "Align right"),
            ] {
                if widgets::icon_button(ui, icon, b, self.text_style.align == align, true, label).clicked() {
                    self.text_style.align = align;
                }
            }
            widgets::vertical_divider(ui, 30.0);
            let bg = self.text_background;
            if widgets::icon_button(ui, Icon::TextBackground, b, bg, true, "Fill background with Color 2").clicked() {
                self.text_background = !bg;
            }
        });
        self.refresh_text(false);
    }

    /// Typing into the active text box. Consumes the keys it handles.
    pub(crate) fn handle_text_keys(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|i| i.events.clone());
        let mut handled = Vec::new();
        let mut commit = false;
        for (index, event) in events.iter().enumerate() {
            let Some(tb) = self.text.as_mut() else {
                return;
            };
            let (start, end) = tb.selection();
            let insert = |tb: &mut super::tools::TextBox, s: &str| {
                let (a, b) = tb.selection();
                tb.text.replace_range(a..b, s);
                tb.cursor = a + s.len();
                tb.anchor = tb.cursor;
            };
            match event {
                Event::Text(t) => {
                    let clean: String = t.chars().filter(|c| !c.is_control()).collect();
                    if !clean.is_empty() {
                        insert(tb, &clean);
                    }
                    handled.push(index);
                }
                Event::Ime(egui::ImeEvent::Commit(t)) => {
                    insert(tb, t);
                    handled.push(index);
                }
                Event::Paste(t) => {
                    insert(tb, &t.replace("\r\n", "\n"));
                    handled.push(index);
                }
                Event::Copy | Event::Cut => {
                    if start != end {
                        ctx.copy_text(tb.text[start..end].to_owned());
                        if matches!(event, Event::Cut) {
                            insert(tb, "");
                        }
                    }
                    handled.push(index);
                }
                Event::Key { key, pressed: true, modifiers, .. } => {
                    let word = if cfg!(target_os = "macos") { modifiers.alt } else { modifiers.ctrl };
                    let text = tb.text.clone();
                    let move_to = |tb: &mut super::tools::TextBox, pos: usize| {
                        tb.cursor = pos;
                        if !modifiers.shift {
                            tb.anchor = pos;
                        }
                    };
                    match key {
                        Key::Backspace if !modifiers.command => {
                            if start == end && start > 0 {
                                tb.anchor = if word { prev_word(&text, start) } else { prev_boundary(&text, start) };
                            }
                            insert(tb, "");
                        }
                        Key::Delete => {
                            if start == end && end < text.len() {
                                tb.anchor = if word { next_word(&text, end) } else { next_boundary(&text, end) };
                            }
                            insert(tb, "");
                        }
                        Key::Enter => insert(tb, "\n"),
                        Key::Tab => insert(tb, "    "),
                        Key::ArrowLeft => {
                            let pos = if start != end && !modifiers.shift {
                                start
                            } else if word {
                                prev_word(&text, tb.cursor)
                            } else {
                                prev_boundary(&text, tb.cursor)
                            };
                            move_to(tb, pos);
                        }
                        Key::ArrowRight => {
                            let pos = if start != end && !modifiers.shift {
                                end
                            } else if word {
                                next_word(&text, tb.cursor)
                            } else {
                                next_boundary(&text, tb.cursor)
                            };
                            move_to(tb, pos);
                        }
                        Key::Home => move_to(tb, line_start(&text, tb.cursor)),
                        Key::End => move_to(tb, line_end(&text, tb.cursor)),
                        Key::ArrowUp | Key::ArrowDown => {
                            let (cursor, rect, swap) = (tb.cursor, tb.rect, tb.swap_colors);
                            let style = self.current_text_style(swap);
                            let engine = self.fonts.get();
                            let (x, y) = engine.caret(&text, &style, rect.width(), cursor);
                            let line = engine.line_height(&style);
                            let target_y = if *key == Key::ArrowUp { y - line / 2.0 } else { y + line * 1.5 };
                            let pos = if target_y < 0.0 { 0 } else { engine.hit(&text, &style, rect.width(), x, target_y) };
                            if let Some(tb) = self.text.as_mut() {
                                move_to(tb, pos);
                            }
                        }
                        Key::A if modifiers.command => {
                            tb.anchor = 0;
                            tb.cursor = text.len();
                        }
                        Key::B if modifiers.command => self.text_style.bold = !self.text_style.bold,
                        Key::I if modifiers.command => self.text_style.italic = !self.text_style.italic,
                        Key::U if modifiers.command => self.text_style.underline = !self.text_style.underline,
                        Key::Escape => commit = true,
                        _ => continue,
                    }
                    handled.push(index);
                }
                _ => {}
            }
        }
        if !handled.is_empty() {
            // Drop the handled events so global shortcuts don't also act on them.
            ctx.input_mut(|i| {
                let mut n = 0;
                i.events.retain(|_| {
                    let keep = !handled.contains(&n);
                    n += 1;
                    keep
                });
            });
            self.refresh_text(false);
        }
        if commit {
            self.commit_text();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundaries_respect_utf8() {
        let s = "aé😀b";
        assert_eq!(next_boundary(s, 0), 1);
        assert_eq!(next_boundary(s, 1), 3);
        assert_eq!(next_boundary(s, 3), 7);
        assert_eq!(prev_boundary(s, 7), 3);
        assert_eq!(prev_boundary(s, 0), 0);
    }

    #[test]
    fn words_and_lines() {
        let s = "hello big world\nnext";
        assert_eq!(next_word(s, 0), 6);
        assert_eq!(prev_word(s, 9), 6);
        assert_eq!(line_start(s, 18), 16);
        assert_eq!(line_end(s, 2), 15);
    }
}
