// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Every user-invokable action, and the keyboard shortcuts that trigger them.

use egui::{Key, KeyboardShortcut, Modifiers};

use super::state::{Tool, next_zoom_in, next_zoom_out};
use super::{PaintApp, dialogs::Dialog, files::PendingAction};
use crate::engine::io::FileFormat;
use crate::ui::theme::ThemePref;

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    // File
    New,
    Open,
    ImportToCanvas,
    OpenRecent(usize),
    ClearRecent,
    Save,
    SaveAs(Option<FileFormat>),
    Print,
    Properties,
    About,
    Exit,
    // Edit
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    PasteFrom,
    SelectAll,
    InvertSelection,
    DeleteSelection,
    ToggleTransparentSelection,
    Crop,
    ResizeSkew,
    RotateRight,
    RotateLeft,
    Rotate180,
    FlipVertical,
    FlipHorizontal,
    // View
    ZoomIn,
    ZoomOut,
    ZoomActual,
    ZoomFit,
    ToggleRulers,
    ToggleGrid,
    ToggleStatusBar,
    ToggleThumbnail,
    ToggleFullscreen,
    ToggleLayers,
    // Layers
    AddLayer,
    DuplicateLayer,
    DeleteLayer,
    MergeDown,
    MoveLayerUp,
    MoveLayerDown,
    // Misc
    EditColors,
    SetTheme(ThemePref),
    IncreaseSize,
    DecreaseSize,
    /// Finish the current shape, text or selection (Enter / Esc).
    Commit,
}

/// Platform-appropriate label for a shortcut, e.g. "Ctrl+S" or "⌘S".
pub fn shortcut_label(modifiers: Modifiers, key: &str) -> String {
    if cfg!(target_os = "macos") && crate::ui::theme::has_mac_symbols() {
        let mut s = String::new();
        if modifiers.ctrl {
            s.push('⌃');
        }
        if modifiers.alt {
            s.push('⌥');
        }
        if modifiers.shift {
            s.push('⇧');
        }
        if modifiers.command || modifiers.mac_cmd {
            s.push('⌘');
        }
        s.push_str(key);
        s
    } else {
        let mut s = String::new();
        if cfg!(target_os = "macos") && (modifiers.command || modifiers.mac_cmd) {
            s.push_str("Cmd+");
        } else if modifiers.command || modifiers.ctrl {
            s.push_str("Ctrl+");
        }
        if modifiers.alt {
            s.push_str("Alt+");
        }
        if modifiers.shift {
            s.push_str("Shift+");
        }
        s.push_str(key);
        s
    }
}

pub fn cmd(key: &str) -> String {
    shortcut_label(Modifiers::COMMAND, key)
}

pub fn cmd_shift(key: &str) -> String {
    shortcut_label(Modifiers::COMMAND | Modifiers::SHIFT, key)
}

impl PaintApp {
    pub fn has_selection(&self) -> bool {
        self.selection.is_some()
    }

    /// Whether a command can run right now (used to grey out menu items).
    pub fn can_execute(&self, command: &Command) -> bool {
        match command {
            Command::Undo => self.history.can_undo() || self.selection.as_ref().is_some_and(|s| s.sel.is_floating()) || self.shape.is_some(),
            Command::Redo => self.history.can_redo(),
            Command::Cut | Command::Copy | Command::DeleteSelection | Command::Crop => self.has_selection(),
            Command::InvertSelection => self.selection.as_ref().is_some_and(|s| !s.sel.is_floating()),
            Command::OpenRecent(i) => *i < self.recent.len(),
            Command::ClearRecent => !self.recent.is_empty(),
            Command::DeleteLayer => self.doc.layers.len() > 1,
            Command::MergeDown | Command::MoveLayerDown => self.doc.active > 0,
            Command::MoveLayerUp => self.doc.active + 1 < self.doc.layers.len(),
            _ => true,
        }
    }

    pub fn execute(&mut self, ctx: &egui::Context, command: Command) {
        if !self.can_execute(&command) {
            return;
        }
        let viewport = self.viewport;
        match command {
            Command::New => self.request(PendingAction::New),
            Command::Open => self.request(PendingAction::Open(None)),
            Command::OpenRecent(i) => {
                if let Some(path) = self.recent.get(i).cloned() {
                    self.request(PendingAction::Open(Some(path)));
                }
            }
            Command::ClearRecent => self.recent.clear(),
            Command::ImportToCanvas | Command::PasteFrom => self.import_to_canvas(),
            Command::Save => {
                self.save();
            }
            Command::SaveAs(format) => {
                self.save_as(format);
            }
            Command::Print => self.open_print_dialog(),
            Command::Properties => self.dialog = Some(Dialog::properties(self)),
            Command::About => self.dialog = Some(Dialog::About),
            Command::Exit => self.request(PendingAction::Exit),

            Command::Undo => self.undo(),
            Command::Redo => self.redo(),
            Command::Cut => self.cut(),
            Command::Copy => self.copy(),
            Command::Paste => self.paste(),
            Command::SelectAll => self.select_all(),
            Command::InvertSelection => self.invert_selection(),
            Command::DeleteSelection => self.delete_selection(),
            Command::ToggleTransparentSelection => {
                self.transparent_selection = !self.transparent_selection;
                if let Some(area) = self.selection.as_ref().and_then(|s| s.sel.floating_area()) {
                    self.invalidate(area);
                }
            }
            Command::Crop => self.crop(),
            Command::ResizeSkew => self.dialog = Some(Dialog::resize_skew(self)),
            Command::RotateRight => self.transform_image("Rotate right", crate::engine::transform::rotate_cw),
            Command::RotateLeft => self.transform_image("Rotate left", crate::engine::transform::rotate_ccw),
            Command::Rotate180 => self.transform_image("Rotate 180°", crate::engine::transform::rotate_180),
            Command::FlipVertical => self.transform_image("Flip vertical", crate::engine::transform::flip_vertical),
            Command::FlipHorizontal => self.transform_image("Flip horizontal", crate::engine::transform::flip_horizontal),

            Command::ZoomIn => self.zoom_to(next_zoom_in(self.view.zoom), None, viewport),
            Command::ZoomOut => self.zoom_to(next_zoom_out(self.view.zoom), None, viewport),
            Command::ZoomActual => self.zoom_to(1.0, None, viewport),
            Command::ZoomFit => self.view.fit_pending = true,
            Command::ToggleRulers => self.view.show_rulers = !self.view.show_rulers,
            Command::ToggleGrid => self.view.show_grid = !self.view.show_grid,
            Command::ToggleStatusBar => self.view.show_status_bar = !self.view.show_status_bar,
            Command::ToggleThumbnail => self.view.show_thumbnail = !self.view.show_thumbnail,
            Command::ToggleLayers => self.view.show_layers = !self.view.show_layers,
            Command::ToggleFullscreen => {
                self.commit_all();
                self.view.fullscreen = !self.view.fullscreen;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.view.fullscreen));
                self.view.fit_pending = true;
            }

            Command::AddLayer => self.add_layer(),
            Command::DuplicateLayer => self.duplicate_layer(self.doc.active),
            Command::DeleteLayer => self.delete_layer(self.doc.active),
            Command::MergeDown => self.merge_down(self.doc.active),
            Command::MoveLayerUp => self.move_layer(self.doc.active, self.doc.active + 1),
            Command::MoveLayerDown => self.move_layer(self.doc.active, self.doc.active.saturating_sub(1)),

            Command::EditColors => self.dialog = Some(Dialog::edit_colors(self)),
            Command::SetTheme(theme) => {
                self.theme = theme;
                self.applied_dark = None;
            }
            Command::IncreaseSize | Command::DecreaseSize => {
                if let Some(kind) = self.tool.size_kind() {
                    let current = self.sizes.get(kind);
                    let next = if command == Command::IncreaseSize { current + 1 } else { current.saturating_sub(1) };
                    self.sizes.set(kind, next);
                }
            }
            Command::Commit => self.commit_all(),
        }
    }

    /// Keyboard shortcuts. Runs before the panels so they can't swallow the keys.
    pub(crate) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.dialog.is_some() {
            return;
        }
        if self.text.is_some() {
            self.handle_text_keys(ctx);
        }
        let typing = self.text.is_some();
        let pressed = |m: Modifiers, k: Key| ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(m, k)));
        let command = Modifiers::COMMAND;
        let command_shift = Modifiers::COMMAND | Modifiers::SHIFT;

        let mut run: Vec<Command> = Vec::new();
        let bindings: &[(Modifiers, Key, Command)] = &[
            (command, Key::N, Command::New),
            (command, Key::O, Command::Open),
            (command_shift, Key::S, Command::SaveAs(None)),
            (command, Key::S, Command::Save),
            (Modifiers::NONE, Key::F12, Command::SaveAs(None)),
            (command, Key::P, Command::Print),
            (command, Key::E, Command::Properties),
            (command, Key::W, Command::ResizeSkew),
            (command_shift, Key::Z, Command::Redo),
            (command, Key::Z, Command::Undo),
            (command, Key::Y, Command::Redo),
            (command_shift, Key::X, Command::Crop),
            (command, Key::R, Command::ToggleRulers),
            (command, Key::G, Command::ToggleGrid),
            (command, Key::PageUp, Command::ZoomIn),
            (command, Key::PageDown, Command::ZoomOut),
            (command, Key::Num0, Command::ZoomFit),
            (command, Key::Num1, Command::ZoomActual),
            (Modifiers::NONE, Key::F11, Command::ToggleFullscreen),
            (command_shift, Key::N, Command::AddLayer),
        ];
        for (m, k, c) in bindings {
            if pressed(*m, *k) {
                run.push(c.clone());
            }
        }
        if !typing {
            let plain: &[(Modifiers, Key, Command)] = &[
                (command, Key::A, Command::SelectAll),
                (command, Key::Plus, Command::IncreaseSize),
                (command, Key::Equals, Command::IncreaseSize),
                (command, Key::Minus, Command::DecreaseSize),
                (Modifiers::NONE, Key::Delete, Command::DeleteSelection),
                (Modifiers::NONE, Key::Enter, Command::Commit),
            ];
            for (m, k, c) in plain {
                if pressed(*m, *k) {
                    run.push(c.clone());
                }
            }
            if pressed(Modifiers::NONE, Key::Escape) {
                if self.view.fullscreen {
                    run.push(Command::ToggleFullscreen);
                } else {
                    run.push(Command::Commit);
                }
            }
            // Clipboard keys arrive as dedicated events.
            let events = ctx.input(|i| i.events.clone());
            for e in events {
                match e {
                    egui::Event::Copy => run.push(Command::Copy),
                    egui::Event::Cut => run.push(Command::Cut),
                    egui::Event::Paste(_) | egui::Event::PasteImage(_) => run.push(Command::Paste),
                    _ => {}
                }
            }
            // Arrow keys nudge a selection by one pixel.
            if self.tool == Tool::Select && self.selection.is_some() {
                let nudge = [(Key::ArrowLeft, -1.0, 0.0), (Key::ArrowRight, 1.0, 0.0), (Key::ArrowUp, 0.0, -1.0), (Key::ArrowDown, 0.0, 1.0)];
                for (k, dx, dy) in nudge {
                    if pressed(Modifiers::NONE, k) {
                        self.move_selection(dx, dy);
                    }
                }
            }
        }
        for c in run {
            self.execute(ctx, c);
        }
    }
}
