// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! New, Open, Save, Import, Print and the "save your changes?" flow.

use std::path::{Path, PathBuf};

use super::dialogs::Dialog;
use super::{FileInfo, PaintApp};
use crate::engine::color::Rgba;
use crate::engine::document::{self, Document};
use crate::engine::io::{self, FileFormat};

const MAX_RECENT: usize = 10;

/// Something that would discard unsaved changes, waiting on the user's answer.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PendingAction {
    New,
    Open(Option<PathBuf>),
    Exit,
}

impl PaintApp {
    /// Runs `action`, first asking to save if there are unsaved changes.
    pub(crate) fn request(&mut self, action: PendingAction) {
        self.commit_all();
        if self.is_dirty() {
            self.dialog = Some(Dialog::ConfirmSave { then: action });
        } else {
            self.perform(action);
        }
    }

    pub(crate) fn perform(&mut self, action: PendingAction) {
        match action {
            PendingAction::New => self.new_document(),
            PendingAction::Open(Some(path)) => {
                self.open_path(&path);
            }
            PendingAction::Open(None) => {
                if let Some(path) = self.pick_open_file("Open") {
                    self.open_path(&path);
                }
            }
            PendingAction::Exit => {
                self.close_confirmed = true;
                if let Some(ctx) = self.ctx.clone() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    pub fn new_document(&mut self) {
        self.selection = None;
        self.shape = None;
        self.text = None;
        self.doc = Document::new(document::DEFAULT_WIDTH, document::DEFAULT_HEIGHT, Rgba::WHITE);
        self.history.clear();
        self.file = FileInfo::default();
        self.view.fit_pending = true;
        self.document_changed();
    }

    pub(crate) fn load_document(&mut self, path: &Path) -> Result<(), String> {
        let pixels = io::load(path)?;
        self.selection = None;
        self.shape = None;
        self.text = None;
        self.doc = Document::from_pixmap(pixels);
        self.history.clear();
        let meta = std::fs::metadata(path).ok();
        self.file = FileInfo {
            path: Some(path.to_path_buf()),
            format: FileFormat::from_path(path),
            saved_state: self.history.state_id(),
            size_on_disk: meta.as_ref().map(|m| m.len()),
            modified: meta.and_then(|m| m.modified().ok()),
        };
        self.add_recent(path);
        self.view.fit_pending = true;
        self.document_changed();
        Ok(())
    }

    /// Opens an image file, showing an error dialog on failure.
    pub fn open_path(&mut self, path: &Path) -> bool {
        match self.load_document(path) {
            Ok(()) => true,
            Err(message) => {
                self.recent.retain(|p| p != path);
                self.dialog = Some(Dialog::message("Couldn't open this picture", &message));
                false
            }
        }
    }

    pub(crate) fn add_recent(&mut self, path: &Path) {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        self.recent.retain(|p| p != &path);
        self.recent.insert(0, path);
        self.recent.truncate(MAX_RECENT);
    }

    fn open_filter(dialog: rfd::FileDialog) -> rfd::FileDialog {
        let all = io::openable_extensions();
        let mut dialog = dialog.add_filter("All picture files", &all);
        for f in FileFormat::ALL {
            dialog = dialog.add_filter(f.label(), f.extensions());
        }
        dialog
    }

    fn start_dir(&self) -> Option<PathBuf> {
        self.file
            .path
            .as_ref()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .or_else(|| self.recent.first().and_then(|p| p.parent().map(Path::to_path_buf)))
    }

    fn pick_open_file(&self, title: &str) -> Option<PathBuf> {
        let mut dialog = Self::open_filter(rfd::FileDialog::new().set_title(title));
        if let Some(dir) = self.start_dir() {
            dialog = dialog.set_directory(dir);
        }
        dialog.pick_file()
    }

    /// "Import to canvas": pastes a picture from a file as a floating selection.
    pub fn import_to_canvas(&mut self) {
        let Some(path) = self.pick_open_file("Import to canvas") else {
            return;
        };
        match io::load(&path) {
            Ok(image) => self.paste_image(image),
            Err(message) => self.dialog = Some(Dialog::message("Couldn't import this picture", &message)),
        }
    }

    /// Saves to the current file, or asks where to save. Returns whether it saved.
    pub fn save(&mut self) -> bool {
        match (self.file.path.clone(), self.file.format) {
            (Some(path), Some(format)) => self.save_to(&path, format),
            _ => self.save_as(None),
        }
    }

    pub fn save_as(&mut self, format: Option<FileFormat>) -> bool {
        self.commit_all();
        let format = format.or(self.file.format).unwrap_or(FileFormat::Png);
        let stem = self
            .file
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_owned());
        let mut dialog = rfd::FileDialog::new()
            .set_title("Save as")
            .set_file_name(format!("{stem}.{}", format.extensions()[0]))
            .add_filter(format.label(), format.extensions());
        for f in FileFormat::ALL.into_iter().filter(|f| *f != format) {
            dialog = dialog.add_filter(f.label(), f.extensions());
        }
        if let Some(dir) = self.start_dir() {
            dialog = dialog.set_directory(dir);
        }
        let Some(mut path) = dialog.save_file() else {
            return false;
        };
        let format = match FileFormat::from_path(&path) {
            Some(f) => f,
            None => {
                path.set_extension(format.extensions()[0]);
                format
            }
        };
        self.save_to(&path, format)
    }

    /// Writes the flattened picture to `path`. Returns whether it saved.
    pub fn save_to(&mut self, path: &Path, format: FileFormat) -> bool {
        self.commit_all();
        match io::save(path, &self.doc.flatten(), format) {
            Ok(size) => {
                self.file = FileInfo {
                    path: Some(path.to_path_buf()),
                    format: Some(format),
                    saved_state: self.history.state_id(),
                    size_on_disk: Some(size),
                    modified: std::fs::metadata(path).and_then(|m| m.modified()).ok(),
                };
                self.add_recent(path);
                true
            }
            Err(message) => {
                self.dialog = Some(Dialog::message("Couldn't save this picture", &message));
                false
            }
        }
    }

    /// Keeps the window open while there are unsaved changes, and asks first.
    pub(crate) fn handle_close_request(&mut self, ctx: &egui::Context) {
        self.ctx = Some(ctx.clone());
        if ctx.input(|i| i.viewport().close_requested()) && !self.close_confirmed {
            self.commit_all();
            if self.is_dirty() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                if self.dialog.is_none() {
                    self.dialog = Some(Dialog::ConfirmSave { then: PendingAction::Exit });
                }
            }
        }
    }

    /// Opening a picture by dropping it on the window.
    pub(crate) fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(path) = dropped.into_iter().next().map(|f| f.path().to_path_buf()) {
            self.request(PendingAction::Open(Some(path)));
        }
    }

    pub(crate) fn open_print_dialog(&mut self) {
        self.commit_all();
        self.dialog = Some(Dialog::print());
    }

    /// Sends the picture to the system's printing service.
    pub(crate) fn print(&mut self, printer: Option<String>, copies: u32, fit: bool, landscape: bool) {
        let path = std::env::temp_dir().join(format!("openbrush-print-{}.png", std::process::id()));
        if let Err(message) = io::save(&path, &self.doc.flatten(), FileFormat::Png) {
            self.dialog = Some(Dialog::message("Couldn't print", &message));
            return;
        }
        if let Err(message) = print_file(&path, printer.as_deref(), copies, fit, landscape) {
            self.dialog = Some(Dialog::message("Couldn't print", &message));
        }
    }
}

/// Printers known to CUPS (macOS and Linux). Empty on Windows or without CUPS.
pub(crate) fn list_printers() -> Vec<String> {
    if cfg!(target_os = "windows") {
        return Vec::new();
    }
    let Ok(out) = std::process::Command::new("lpstat").arg("-e").output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout).lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_owned).collect()
}

fn print_file(path: &Path, printer: Option<&str>, copies: u32, fit: bool, landscape: bool) -> Result<(), String> {
    if cfg!(target_os = "windows") {
        // Hands the picture to Windows' own photo printing dialog.
        let script = format!("Start-Process -FilePath '{}' -Verb Print", path.display().to_string().replace('\'', "''"));
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script])
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Windows couldn't start printing: {e}"))
    } else {
        let mut cmd = std::process::Command::new("lp");
        if let Some(p) = printer {
            cmd.args(["-d", p]);
        }
        cmd.args(["-n", &copies.max(1).to_string()]);
        if fit {
            cmd.args(["-o", "fit-to-page"]);
        }
        if landscape {
            cmd.args(["-o", "landscape"]);
        }
        let out = cmd.arg(path).output().map_err(|e| format!("The print service isn't available ({e})."))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
        }
    }
}
