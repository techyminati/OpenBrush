// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! The File / Edit / View menu bar with quick Save, Undo and Redo buttons.

use egui::containers::menu::SubMenu;
use egui::{Align, Layout, Popup, Ui, vec2};

use super::commands::{Command, cmd, cmd_shift};
use super::{APP_NAME, PaintApp};
use crate::engine::io::FileFormat;
use crate::ui::icons::Icon;
use crate::ui::theme::ThemePref;
use crate::ui::widgets::{self, MenuItem, menu_separator};

impl PaintApp {
    /// A menu item that runs `command` when clicked, greyed out when it can't run.
    fn item(&mut self, ui: &mut Ui, item: MenuItem, command: Command) {
        let enabled = self.can_execute(&command);
        if item.enabled(enabled).show(ui).clicked() {
            let ctx = ui.ctx().clone();
            self.execute(&ctx, command);
        }
    }

    fn top_menu(&mut self, ui: &mut Ui, label: &str, content: impl FnOnce(&mut Self, &mut Ui)) {
        let id_probe = ui.next_auto_id();
        let open = Popup::is_id_open(ui.ctx(), id_probe.with("popup"));
        let response = widgets::menu_bar_button(ui, label, open);
        let frame = widgets::popup_frame(ui);
        Popup::menu(&response).frame(frame).gap(2.0).show(|ui| {
            ui.set_min_width(270.0);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            content(self, ui);
        });
    }

    pub(crate) fn menu_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            self.top_menu(ui, "File", Self::file_menu);
            self.top_menu(ui, "Edit", Self::edit_menu);
            self.top_menu(ui, "View", Self::view_menu);
            widgets::vertical_divider(ui, 30.0);
            let ctx = ui.ctx().clone();
            let size = vec2(32.0, 30.0);
            if widgets::icon_button(ui, Icon::Save, size, false, true, &format!("Save ({})", cmd("S"))).clicked() {
                self.execute(&ctx, Command::Save);
            }
            let can_undo = self.can_execute(&Command::Undo);
            if widgets::icon_button(ui, Icon::Undo, size, false, can_undo, &format!("Undo ({})", cmd("Z"))).clicked() {
                self.execute(&ctx, Command::Undo);
            }
            let can_redo = self.can_execute(&Command::Redo);
            if widgets::icon_button(ui, Icon::Redo, size, false, can_redo, &format!("Redo ({})", cmd("Y"))).clicked() {
                self.execute(&ctx, Command::Redo);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let response = widgets::icon_button(ui, Icon::Settings, size, false, true, "Settings");
                let frame = widgets::popup_frame(ui);
                Popup::menu(&response).frame(frame).align(egui::RectAlign::BOTTOM_END).show(|ui| {
                    ui.set_min_width(250.0);
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    for (theme, icon) in [
                        (ThemePref::Light, Icon::ThemeLight),
                        (ThemePref::Dark, Icon::ThemeDark),
                        (ThemePref::System, Icon::ThemeSystem),
                    ] {
                        let checked = self.theme == theme;
                        let label = theme.label();
                        if MenuItem::new(label).icon(icon).checked(checked).show(ui).clicked() {
                            let ctx = ui.ctx().clone();
                            self.execute(&ctx, Command::SetTheme(theme));
                        }
                    }
                    menu_separator(ui);
                    self.item(ui, MenuItem::new(&format!("About {APP_NAME}")).icon(Icon::About), Command::About);
                });
            });
        });
    }

    fn file_menu(&mut self, ui: &mut Ui) {
        self.item(ui, MenuItem::new("New").icon(Icon::New).shortcut(&cmd("N")), Command::New);
        self.item(ui, MenuItem::new("Open").icon(Icon::Open).shortcut(&cmd("O")), Command::Open);
        let r = MenuItem::new("Import to canvas").icon(Icon::Import).submenu().show(ui);
        SubMenu::new().show(ui, &r, |ui| {
            ui.set_min_width(220.0);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            self.item(ui, MenuItem::new("From a file"), Command::ImportToCanvas);
        });
        let r = MenuItem::new("Recent").icon(Icon::Recent).submenu().show(ui);
        SubMenu::new().show(ui, &r, |ui| {
            ui.set_min_width(300.0);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            if self.recent.is_empty() {
                MenuItem::new("No recent pictures").enabled(false).show(ui);
            }
            let names: Vec<String> = self
                .recent
                .iter()
                .map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
                .collect();
            for (i, name) in names.iter().enumerate() {
                let response = MenuItem::new(name).icon(Icon::Properties).show(ui);
                let response = response.on_hover_text(self.recent[i].display().to_string());
                if response.clicked() {
                    let ctx = ui.ctx().clone();
                    self.execute(&ctx, Command::OpenRecent(i));
                }
            }
            if !self.recent.is_empty() {
                menu_separator(ui);
                self.item(ui, MenuItem::new("Clear recent pictures"), Command::ClearRecent);
            }
        });
        self.item(ui, MenuItem::new("Save").icon(Icon::Save).shortcut(&cmd("S")), Command::Save);
        let r = MenuItem::new("Save as").icon(Icon::SaveAs).submenu().show(ui);
        SubMenu::new().show(ui, &r, |ui| {
            ui.set_min_width(240.0);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            for format in FileFormat::ALL {
                self.item(ui, MenuItem::new(format.label()), Command::SaveAs(Some(format)));
            }
            menu_separator(ui);
            self.item(ui, MenuItem::new("Other format…").shortcut("F12"), Command::SaveAs(None));
        });
        menu_separator(ui);
        self.item(ui, MenuItem::new("Print").icon(Icon::Print).shortcut(&cmd("P")), Command::Print);
        self.item(ui, MenuItem::new("Image properties").icon(Icon::Properties).shortcut(&cmd("E")), Command::Properties);
        menu_separator(ui);
        self.item(ui, MenuItem::new(&format!("About {APP_NAME}")).icon(Icon::About), Command::About);
        self.item(ui, MenuItem::new("Exit").icon(Icon::Exit), Command::Exit);
    }

    fn edit_menu(&mut self, ui: &mut Ui) {
        self.item(ui, MenuItem::new("Undo").icon(Icon::Undo).shortcut(&cmd("Z")), Command::Undo);
        self.item(ui, MenuItem::new("Redo").icon(Icon::Redo).shortcut(&cmd("Y")), Command::Redo);
        menu_separator(ui);
        self.item(ui, MenuItem::new("Cut").icon(Icon::Cut).shortcut(&cmd("X")), Command::Cut);
        self.item(ui, MenuItem::new("Copy").icon(Icon::Copy).shortcut(&cmd("C")), Command::Copy);
        self.item(ui, MenuItem::new("Paste").icon(Icon::Paste).shortcut(&cmd("V")), Command::Paste);
        self.item(ui, MenuItem::new("Paste from…").icon(Icon::PasteFrom), Command::PasteFrom);
        menu_separator(ui);
        self.item(ui, MenuItem::new("Select all").icon(Icon::SelectAll).shortcut(&cmd("A")), Command::SelectAll);
        self.item(ui, MenuItem::new("Invert selection").icon(Icon::InvertSelection), Command::InvertSelection);
        self.item(ui, MenuItem::new("Delete").icon(Icon::Delete).shortcut("Del"), Command::DeleteSelection);
        self.item(ui, MenuItem::new("Crop to selection").icon(Icon::Crop).shortcut(&cmd_shift("X")), Command::Crop);
        let transparent = self.transparent_selection;
        self.item(ui, MenuItem::new("Transparent selection").checked(transparent), Command::ToggleTransparentSelection);
    }

    fn view_menu(&mut self, ui: &mut Ui) {
        let r = MenuItem::new("Zoom").icon(Icon::ZoomIn).submenu().show(ui);
        SubMenu::new().show(ui, &r, |ui| {
            ui.set_min_width(240.0);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            self.item(ui, MenuItem::new("Zoom in").icon(Icon::ZoomIn).shortcut(&cmd("PgUp")), Command::ZoomIn);
            self.item(ui, MenuItem::new("Zoom out").icon(Icon::ZoomOut).shortcut(&cmd("PgDn")), Command::ZoomOut);
            self.item(ui, MenuItem::new("Actual size (100%)").icon(Icon::ZoomActual).shortcut(&cmd("1")), Command::ZoomActual);
            self.item(ui, MenuItem::new("Fit to window").icon(Icon::ZoomFit).shortcut(&cmd("0")), Command::ZoomFit);
        });
        let r = MenuItem::new("Show").icon(Icon::Eye).submenu().show(ui);
        SubMenu::new().show(ui, &r, |ui| {
            ui.set_min_width(240.0);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let v = self.view.clone();
            self.item(ui, MenuItem::new("Rulers").checked(v.show_rulers).shortcut(&cmd("R")), Command::ToggleRulers);
            self.item(ui, MenuItem::new("Gridlines").checked(v.show_grid).shortcut(&cmd("G")), Command::ToggleGrid);
            self.item(ui, MenuItem::new("Status bar").checked(v.show_status_bar), Command::ToggleStatusBar);
            self.item(ui, MenuItem::new("Thumbnail").checked(v.show_thumbnail), Command::ToggleThumbnail);
            self.item(ui, MenuItem::new("Layers").checked(v.show_layers), Command::ToggleLayers);
        });
        menu_separator(ui);
        self.item(ui, MenuItem::new("Full screen").icon(Icon::Fullscreen).shortcut("F11"), Command::ToggleFullscreen);
    }
}
