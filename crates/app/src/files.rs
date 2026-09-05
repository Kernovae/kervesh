use crate::{
    app::{App, Confirmation, FileDialog, FileDialogKind, TransferRow},
    classify::FileType,
    icons::{UiIcon, file_icon_texture, paint_file_icon, ui_icon_button},
    theme::colors,
};
use egui::{Color32, Key, Pos2, Rect, RichText, Sense, Vec2};
use kervesh_core::bytes;
use kervesh_ssh::{
    CancellationToken, Command, Direction, FileOperation, TransferRequest, TransferState,
    remote_join,
};
use std::path::PathBuf;

fn format_modified(ts: Option<u32>) -> String {
    let Some(ts) = ts else {
        return "—".into();
    };
    let days = (ts as u64 / 86400) as i64;
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1020 + doe / 1461 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let m_idx = (m as usize).saturating_sub(1).min(11);
    format!("{} {}, {}", MONTHS[m_idx], d, y)
}

impl App {
    pub(crate) fn file_sidebar(&mut self, ctx: &egui::Context) {
        let mut operation = None;
        let mut dialog = None;
        let mut deletion = None;
        let mut upload = false;
        let mut download = None;
        let mut open_search = false;
        let mut open_sync = false;

        egui::SidePanel::right("files")
            .default_width(320.0)
            .width_range(240.0..=650.0)
            .show(ctx, |ui| {
                let Some(tab) = self.tabs.get_mut(self.active) else {
                    return;
                };
                let id = tab.id;
                let dark = self.settings.dark;

                if tab.terminal.profile().follow_terminal_directory {
                    if tab.follow_suspended {
                        if ui
                            .small_button("Resume following terminal directory")
                            .clicked()
                        {
                            tab.follow_suspended = false;
                            tab.last_followed = None;
                        }
                    } else {
                        ui.label(
                            egui::RichText::new(if tab.terminal.directory().is_some() {
                                "Following terminal directory"
                            } else {
                                "Waiting for terminal directory metadata"
                            })
                            .small()
                            .weak(),
                        );
                    }
                }
                // Tab Header: SFTP / File Browser / Bookmarks
                ui.horizontal(|ui| {
                    let _ = ui.selectable_label(true, "SFTP");
                    let _ = ui.selectable_label(false, "File Browser");
                    let _ = ui.selectable_label(false, "Bookmarks");

                    if tab.busy {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spinner();
                        });
                    }
                });
                ui.separator();

                ui.add_enabled_ui(tab.connected, |ui| {
                    // Navigation bar: Back, Forward, Path Edit, Actions
                    ui.horizontal(|ui| {
                        if ui_icon_button(ui, UiIcon::Back, "Back (Alt+Left)", dark).clicked()
                            && let Some(path) = tab.history.pop()
                        {
                            operation = Some((id, FileOperation::List(path)));
                        }
                        if ui_icon_button(ui, UiIcon::Forward, "Forward", dark).clicked() {
                            // Forward placeholder if history stack supports it
                        }
                        if ui_icon_button(ui, UiIcon::Parent, "Parent directory", dark).clicked() {
                            tab.history.push(tab.path.clone());
                            operation = Some((id, FileOperation::List(format!("{}/..", tab.path))));
                        }
                        if ui_icon_button(ui, UiIcon::Refresh, "Refresh", dark).clicked() {
                            operation = Some((id, FileOperation::List(tab.path.clone())));
                        }
                        if ui_icon_button(ui, UiIcon::Upload, "Upload local file…", dark).clicked()
                        {
                            upload = true;
                        }
                        if ui_icon_button(ui, UiIcon::NewFile, "New file", dark).clicked() {
                            dialog = Some(FileDialog {
                                tab: id,
                                kind: FileDialogKind::CreateFile,
                                name: String::new(),
                                error: None,
                            });
                        }
                        if ui_icon_button(ui, UiIcon::NewFolder, "New folder", dark).clicked() {
                            dialog = Some(FileDialog {
                                tab: id,
                                kind: FileDialogKind::CreateDirectory,
                                name: String::new(),
                                error: None,
                            });
                        }
                        if ui_icon_button(ui, UiIcon::Search, "Search files (Grep)", dark).clicked()
                        {
                            open_search = true;
                        }
                        if ui_icon_button(ui, UiIcon::Transfer, "Directory synchronization…", dark)
                            .clicked()
                        {
                            open_sync = true;
                        }
                    });

                    // Direct path edit box
                    let path_resp = ui.add(
                        egui::TextEdit::singleline(&mut tab.path_input)
                            .hint_text("Remote path…")
                            .desired_width(f32::INFINITY),
                    );
                    if path_resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        tab.history.push(tab.path.clone());
                        operation = Some((id, FileOperation::List(tab.path_input.clone())));
                    }

                    // Search / filter files
                    ui.add(
                        egui::TextEdit::singleline(&mut tab.filter)
                            .hint_text("Filter files…")
                            .desired_width(f32::INFINITY),
                    );
                });

                ui.separator();

                // Responsive Columns Header
                let panel_width = ui.available_width();
                let show_size = panel_width >= 260.0;
                let show_modified = panel_width >= 360.0;

                ui.horizontal(|ui| {
                    ui.add_space(26.0);
                    ui.label(RichText::new("Name").small().strong().weak());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if show_modified {
                            ui.label(RichText::new("Modified").small().strong().weak());
                            ui.add_space(20.0);
                        }
                        if show_size {
                            ui.label(RichText::new("Size").small().strong().weak());
                            ui.add_space(10.0);
                        }
                    });
                });
                ui.separator();

                let text_color = if dark {
                    colors::FOREGROUND
                } else {
                    colors::LIGHT_FOREGROUND
                };

                // File rows list
                egui::ScrollArea::vertical()
                    .id_salt("remote_files")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // Parent directory entry '..'
                        if tab.path != "/" && !tab.path.is_empty() {
                            let mut item_rect = ui.available_rect_before_wrap();
                            item_rect.set_height(24.0);
                            let response = ui.allocate_rect(item_rect, Sense::click());
                            if response.hovered() {
                                let bg = if dark {
                                    colors::GRAPHITE
                                } else {
                                    colors::LIGHT_PANEL
                                };
                                ui.painter().rect_filled(item_rect, 4.0, bg);
                            }
                            // Up arrow
                            ui.painter().text(
                                Pos2::new(item_rect.min.x + 8.0, item_rect.min.y + 3.0),
                                egui::Align2::LEFT_TOP,
                                "↑",
                                egui::FontId::proportional(14.0),
                                text_color,
                            );
                            ui.painter().text(
                                Pos2::new(item_rect.min.x + 28.0, item_rect.min.y + 3.0),
                                egui::Align2::LEFT_TOP,
                                "..",
                                egui::FontId::proportional(13.0),
                                text_color,
                            );
                            if response.double_clicked() {
                                tab.history.push(tab.path.clone());
                                operation =
                                    Some((id, FileOperation::List(format!("{}/..", tab.path))));
                            }
                            ui.allocate_space(Vec2::new(0.0, 2.0));
                        }

                        for entry in &tab.entries {
                            if (!self.settings.show_hidden && entry.name.starts_with('.'))
                                || (!tab.filter.is_empty()
                                    && !entry
                                        .name
                                        .to_lowercase()
                                        .contains(&tab.filter.to_lowercase()))
                            {
                                continue;
                            }

                            let file_type = FileType::classify(entry);
                            let is_selected =
                                tab.selected.as_ref().is_some_and(|s| s.name == entry.name);

                            let mut item_rect = ui.available_rect_before_wrap();
                            item_rect.set_height(24.0);
                            let response = ui.allocate_rect(item_rect, Sense::click());

                            if response.clicked() {
                                tab.selected = Some(entry.clone());
                            }

                            if is_selected {
                                let bg = if dark {
                                    colors::SLATE
                                } else {
                                    colors::LIGHT_BORDER
                                };
                                ui.painter().rect_filled(item_rect, 4.0, bg);
                            } else if response.hovered() {
                                let bg = if dark {
                                    colors::GRAPHITE
                                } else {
                                    colors::LIGHT_PANEL
                                };
                                ui.painter().rect_filled(item_rect, 4.0, bg);
                            }

                            // Render icon
                            let icon_rect = Rect::from_min_size(
                                Pos2::new(item_rect.min.x + 6.0, item_rect.min.y + 4.0),
                                Vec2::splat(16.0),
                            );
                            paint_file_icon(ui.painter(), icon_rect, file_type, text_color);

                            // Calculate column text bounds
                            let mut text_end = item_rect.max.x - 6.0;

                            if show_modified {
                                let date_str = format_modified(entry.modified);
                                let date_pos =
                                    Pos2::new(item_rect.max.x - 6.0, item_rect.min.y + 3.0);
                                ui.painter().text(
                                    date_pos,
                                    egui::Align2::RIGHT_TOP,
                                    &date_str,
                                    egui::FontId::proportional(12.0),
                                    colors::MUTED,
                                );
                                text_end -= 85.0;
                            }

                            if show_size {
                                let size_str = if entry.directory {
                                    "—".into()
                                } else {
                                    bytes(entry.size)
                                };
                                let size_pos = Pos2::new(text_end - 10.0, item_rect.min.y + 3.0);
                                ui.painter().text(
                                    size_pos,
                                    egui::Align2::RIGHT_TOP,
                                    &size_str,
                                    egui::FontId::proportional(12.0),
                                    colors::MUTED,
                                );
                                text_end -= 60.0;
                            }

                            // Render name with clipping
                            let label_rect = Rect::from_min_max(
                                Pos2::new(item_rect.min.x + 28.0, item_rect.min.y + 3.0),
                                Pos2::new(
                                    text_end.max(item_rect.min.x + 30.0),
                                    item_rect.max.y - 3.0,
                                ),
                            );
                            ui.painter().with_clip_rect(label_rect).text(
                                label_rect.min,
                                egui::Align2::LEFT_TOP,
                                &entry.name,
                                egui::FontId::proportional(13.0),
                                if is_selected {
                                    colors::WHITE
                                } else {
                                    text_color
                                },
                            );

                            let path = match remote_join(&tab.path, &entry.name) {
                                Ok(path) => path,
                                Err(_) => continue,
                            };

                            if response.double_clicked() {
                                if entry.directory || entry.symlink {
                                    tab.history.push(tab.path.clone());
                                    operation = Some((id, FileOperation::List(path.clone())));
                                } else {
                                    operation = Some((id, FileOperation::Read(path.clone())));
                                }
                            }

                            // Tooltip with comprehensive metadata
                            response
                                .on_hover_ui(|ui| {
                                    ui.label(RichText::new(&path).monospace().strong());
                                    ui.separator();
                                    ui.label(format!("Type: {}", file_type.label()));
                                    if !entry.directory {
                                        ui.label(format!("Size: {}", bytes(entry.size)));
                                    }
                                    ui.label(format!(
                                        "Modified: {}",
                                        entry
                                            .modified
                                            .map(|v| format!(
                                                "{v} (Unix seconds) · {}",
                                                format_modified(Some(v))
                                            ))
                                            .unwrap_or_else(|| "unknown".into())
                                    ));
                                    ui.label(format!(
                                        "Owner: {}:{}",
                                        entry
                                            .uid
                                            .map(|v| v.to_string())
                                            .unwrap_or_else(|| "?".into()),
                                        entry
                                            .gid
                                            .map(|v| v.to_string())
                                            .unwrap_or_else(|| "?".into())
                                    ));
                                    ui.label(format!(
                                        "Permissions: {:04o}",
                                        entry.permissions.unwrap_or(0) & 0o7777
                                    ));
                                })
                                .context_menu(|ui| {
                                    if entry.directory && ui.button("Open").clicked() {
                                        tab.history.push(tab.path.clone());
                                        operation = Some((id, FileOperation::List(path.clone())));
                                        ui.close();
                                    }
                                    if !entry.directory && !entry.symlink {
                                        if ui.button("Edit…").clicked() {
                                            operation =
                                                Some((id, FileOperation::Read(path.clone())));
                                            ui.close();
                                        }
                                        if ui.button("Download…").clicked() {
                                            download = Some((id, path.clone(), entry.name.clone()));
                                            ui.close();
                                        }
                                    }
                                    ui.separator();
                                    if ui.button("Copy remote path").clicked() {
                                        ui.ctx().copy_text(path.clone());
                                        ui.close();
                                    }
                                    if ui.button("Rename…").clicked() {
                                        dialog = Some(FileDialog {
                                            tab: id,
                                            kind: FileDialogKind::Rename(path.clone()),
                                            name: entry.name.clone(),
                                            error: None,
                                        });
                                        ui.close();
                                    }
                                    if ui.button("Permissions…").clicked() {
                                        dialog = Some(FileDialog {
                                            tab: id,
                                            kind: FileDialogKind::Permissions(path.clone()),
                                            name: format!(
                                                "{:o}",
                                                entry.permissions.unwrap_or(0) & 0o7777
                                            ),
                                            error: None,
                                        });
                                        ui.close();
                                    }
                                    ui.separator();
                                    if ui
                                        .add(egui::Button::new(
                                            RichText::new("Delete…").color(colors::DANGER),
                                        ))
                                        .clicked()
                                    {
                                        deletion = Some(Confirmation::File(
                                            id,
                                            FileOperation::Delete(path.clone(), entry.directory),
                                        ));
                                        ui.close();
                                    }
                                });

                            ui.allocate_space(Vec2::new(0.0, 2.0));
                        }

                        // Keyboard navigation handling for selected file
                        if let Some(selected) = &tab.selected {
                            if ui.input(|i| i.key_pressed(Key::Enter)) {
                                let path =
                                    remote_join(&tab.path, &selected.name).unwrap_or_default();
                                if selected.directory || selected.symlink {
                                    tab.history.push(tab.path.clone());
                                    operation = Some((id, FileOperation::List(path)));
                                } else {
                                    operation = Some((id, FileOperation::Read(path)));
                                }
                            }
                            if ui.input(|i| i.key_pressed(Key::F2)) {
                                let path =
                                    remote_join(&tab.path, &selected.name).unwrap_or_default();
                                dialog = Some(FileDialog {
                                    tab: id,
                                    kind: FileDialogKind::Rename(path),
                                    name: selected.name.clone(),
                                    error: None,
                                });
                            }
                            if ui.input(|i| i.key_pressed(Key::Delete)) {
                                let path =
                                    remote_join(&tab.path, &selected.name).unwrap_or_default();
                                deletion = Some(Confirmation::File(
                                    id,
                                    FileOperation::Delete(path, selected.directory),
                                ));
                            }
                            if ui.input(|i| i.modifiers.command && i.key_pressed(Key::C))
                                && let Ok(path) = remote_join(&tab.path, &selected.name)
                            {
                                ui.ctx().copy_text(path);
                            }
                        }

                        // Alt+Left or Backspace to go back
                        if (ui.input(|i| i.modifiers.alt && i.key_pressed(Key::ArrowLeft))
                            || ui.input(|i| i.key_pressed(Key::Backspace)))
                            && let Some(path) = tab.history.pop()
                        {
                            operation = Some((id, FileOperation::List(path)));
                        }

                        if tab.entries.is_empty() && !tab.busy {
                            ui.label(RichText::new("Directory empty or unavailable").weak());
                        }
                    });
            });

        if let Some((id, operation)) = operation {
            if matches!(operation, FileOperation::List(_))
                && let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id)
            {
                tab.follow_suspended = true;
            }
            if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                tab.busy = true;
            }
            self.send(id, Command::File(operation));
        }
        if let Some(dialog) = dialog {
            self.file_dialog = Some(dialog);
        }
        if let Some(confirm) = deletion {
            self.confirmation = Some(confirm);
        }
        if upload
            && let Some(path) = rfd::FileDialog::new().pick_file()
            && let Some(tab) = self.tabs.get(self.active)
        {
            self.prepare_upload(tab.id, path);
        }
        if let Some((id, remote, name)) = download
            && let Some(local) = rfd::FileDialog::new().set_file_name(name).save_file()
        {
            self.prepare_download(id, remote, local);
        }
        if open_search && let Some(tab) = self.tabs.get(self.active) {
            self.search_ui.open_for_path(tab.path.clone());
        }
        if open_sync && let Some(tab) = self.tabs.get(self.active) {
            self.sync_ui.open_for_remote(tab.path.clone());
        }
    }

    pub(crate) fn file_action_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.file_dialog.take() else {
            return;
        };
        let mut open = true;
        let mut apply = false;
        let title = match dialog.kind {
            FileDialogKind::CreateFile => "New remote file",
            FileDialogKind::CreateDirectory => "New remote directory",
            FileDialogKind::Rename(_) => "Rename remote item",
            FileDialogKind::Permissions(_) => "Remote permissions",
        };
        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                if let FileDialogKind::Rename(path) | FileDialogKind::Permissions(path) =
                    &dialog.kind
                {
                    ui.monospace(path);
                }
                ui.label(if matches!(dialog.kind, FileDialogKind::Permissions(_)) {
                    "Octal permissions (e.g. 644)"
                } else {
                    "Name"
                });
                ui.text_edit_singleline(&mut dialog.name);
                if let Some(error) = &dialog.error {
                    ui.colored_label(colors::DANGER, error);
                }
                apply = ui.button("Apply").clicked();
            });
        if apply && let Some(tab) = self.tabs.iter().find(|t| t.id == dialog.tab) {
            let result = (|| -> anyhow::Result<FileOperation> {
                Ok(match &dialog.kind {
                    FileDialogKind::CreateFile => {
                        FileOperation::CreateFile(remote_join(&tab.path, &dialog.name)?)
                    }
                    FileDialogKind::CreateDirectory => {
                        FileOperation::CreateDirectory(remote_join(&tab.path, &dialog.name)?)
                    }
                    FileDialogKind::Rename(from) => {
                        FileOperation::Rename(from.clone(), remote_join(&tab.path, &dialog.name)?)
                    }
                    FileDialogKind::Permissions(path) => {
                        let mode = u32::from_str_radix(&dialog.name, 8)?;
                        anyhow::ensure!(mode <= 0o7777, "Permissions must be 0000–7777");
                        FileOperation::Permissions(path.clone(), mode)
                    }
                })
            })();
            match result {
                Ok(operation) => {
                    self.send(dialog.tab, Command::File(operation));
                    open = false;
                }
                Err(e) => dialog.error = Some(e.to_string()),
            }
        }
        if open {
            self.file_dialog = Some(dialog);
        }
    }

    pub(crate) fn file_editor_window(&mut self, ctx: &egui::Context) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        let Some(editor) = &mut tab.editor else {
            return;
        };
        let mut open = true;
        let mut save = false;
        let tab_id = tab.id;
        let title = format!(
            "Edit — {} ({}){}",
            editor.name,
            editor.syntax.as_str(),
            if editor.dirty { " *" } else { "" }
        );

        let dark = self.settings.dark;
        let mut save_as_path: Option<String> = None;

        egui::Window::new(title)
            .open(&mut open)
            .default_width(760.0)
            .default_height(540.0)
            .resizable(true)
            .vscroll(false)
            .show(ctx, |ui| {
                // Top control bar
                ui.horizontal(|ui| {
                    ui.monospace(&editor.path);
                    ui.separator();

                    if editor.saving {
                        ui.spinner();
                        ui.label("Saving…");
                    } else {
                        if ui.button("💾 Save").clicked()
                            || (ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)))
                        {
                            save = true;
                        }
                        if ui.button("Save As…").clicked() {
                            editor.save_as_open = !editor.save_as_open;
                        }
                    }

                    if ui.button("🔍 Find/Replace").clicked() {
                        editor.search_open = !editor.search_open;
                    }

                    // Line endings switch
                    let ending_label = editor.line_ending.as_str();
                    egui::ComboBox::from_id_salt("editor_line_ending")
                        .selected_text(ending_label)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut editor.line_ending,
                                crate::editor::LineEnding::Lf,
                                "LF (Unix)",
                            );
                            ui.selectable_value(
                                &mut editor.line_ending,
                                crate::editor::LineEnding::Crlf,
                                "CRLF (Windows)",
                            );
                        });

                    ui.label(
                        RichText::new(format!(
                            "{} lines, {} bytes",
                            editor.line_count(),
                            editor.content.len()
                        ))
                        .weak()
                        .small(),
                    );

                    if editor.dirty {
                        ui.colored_label(colors::WARNING, "● Unsaved");
                    }
                });

                // Save As bar
                if editor.save_as_open {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label("Save as path:");
                            ui.add(
                                egui::TextEdit::singleline(&mut editor.save_as_input)
                                    .desired_width(320.0),
                            );
                            if ui.button("Save to new path").clicked()
                                && !editor.save_as_input.trim().is_empty()
                            {
                                save_as_path = Some(editor.save_as_input.trim().to_string());
                                editor.save_as_open = false;
                            }
                            if ui.button("Cancel").clicked() {
                                editor.save_as_open = false;
                            }
                        });
                    });
                }

                // Find and Replace toolbar
                if editor.search_open {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label("Find:");
                            ui.add(
                                egui::TextEdit::singleline(&mut editor.search_query)
                                    .desired_width(180.0),
                            );
                            ui.label("Replace:");
                            ui.add(
                                egui::TextEdit::singleline(&mut editor.replace_query)
                                    .desired_width(180.0),
                            );
                            ui.checkbox(&mut editor.case_sensitive, "Match Case");

                            let matches = editor.count_search_matches();
                            ui.label(RichText::new(format!("{matches} matches")).weak().small());

                            if ui.button("Replace Next").clicked() {
                                editor.replace_next();
                            }
                            if ui.button("Replace All").clicked() {
                                editor.replace_all();
                            }
                        });
                    });
                }

                if let Some(error) = &editor.error {
                    ui.colored_label(colors::DANGER, error);
                }

                ui.separator();

                // Main code editing area with line numbers
                let line_count = editor.line_count();
                let line_numbers: String = (1..=line_count)
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join("\n");

                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            // Line numbers column
                            ui.add(
                                egui::Label::new(RichText::new(&line_numbers).monospace().color(
                                    if dark {
                                        Color32::from_rgb(100, 110, 130)
                                    } else {
                                        Color32::from_rgb(160, 160, 170)
                                    },
                                ))
                                .selectable(false),
                            );
                            ui.separator();

                            // Code editor body
                            let text_edit = egui::TextEdit::multiline(&mut editor.content)
                                .font(egui::TextStyle::Monospace)
                                .code_editor()
                                .desired_rows(24)
                                .desired_width(f32::INFINITY);
                            let response = ui.add(text_edit);
                            if response.changed() {
                                editor.dirty = true;
                            }
                        });
                    });
            });

        if let Some(new_path) = save_as_path {
            editor.path = new_path.clone();
            editor.name = new_path.rsplit('/').next().unwrap_or(&new_path).to_string();
            save = true;
        }

        let to_save = if save {
            editor.saving = true;
            editor.error = None;
            let final_content = editor.prepare_save_content();
            Some((editor.path.clone(), final_content))
        } else {
            None
        };
        if !open {
            tab.editor = None;
        }
        if let Some((path, content)) = to_save {
            self.send(tab_id, Command::File(FileOperation::Write(path, content)));
        }
    }

    pub(crate) fn prepare_upload(&mut self, id: u64, local: PathBuf) {
        let Some(tab) = self.tabs.iter().find(|t| t.id == id) else {
            return;
        };
        let Some(name) = local.file_name().and_then(|n| n.to_str()) else {
            self.notice = Some("File name must be valid UTF-8".into());
            return;
        };
        if !local.is_file() {
            self.notice = Some(
                "Choose a regular file; recursive folder transfer is not available in v0.1".into(),
            );
            return;
        }
        let remote = match remote_join(&tab.path, name) {
            Ok(path) => path,
            Err(e) => {
                self.notice = Some(e.to_string());
                return;
            }
        };
        let exists = tab.entries.iter().any(|e| e.name == name);
        self.next_id += 1;
        let request = TransferRequest {
            id: self.next_id,
            direction: Direction::Upload,
            local,
            remote,
            overwrite: false,
            cancel: CancellationToken::new(),
        };
        if exists {
            self.confirmation = Some(Confirmation::Transfer(id, request));
        } else {
            self.queue_transfer(id, request);
        }
    }

    fn prepare_download(&mut self, id: u64, remote: String, local: PathBuf) {
        let exists = local.exists();
        self.next_id += 1;
        let request = TransferRequest {
            id: self.next_id,
            direction: Direction::Download,
            local,
            remote,
            overwrite: false,
            cancel: CancellationToken::new(),
        };
        if exists {
            self.confirmation = Some(Confirmation::Transfer(id, request));
        } else {
            self.queue_transfer(id, request);
        }
    }

    pub(crate) fn queue_transfer(&mut self, id: u64, request: TransferRequest) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.transfers.push(TransferRow {
                request: request.clone(),
                state: TransferState::Queued,
                done: 0,
                total: 0,
                speed: 0.0,
            });
            self.send(id, Command::Transfer(request));
            self.transfers_open = true;
        }
    }

    pub(crate) fn transfer_panel(&mut self, ctx: &egui::Context) {
        let mut retries = Vec::new();
        let mut clear_completed = false;
        let dark = self.settings.dark;

        egui::TopBottomPanel::bottom("transfers")
            .resizable(true)
            .default_height(140.0)
            .height_range(90.0..=350.0)
            .show(ctx, |ui| {
                let mut active_count = 0;
                let mut completed_count = 0;
                for tab in &self.tabs {
                    for row in &tab.transfers {
                        match row.state {
                            TransferState::Running | TransferState::Queued => active_count += 1,
                            TransferState::Complete => completed_count += 1,
                            _ => {}
                        }
                    }
                }

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Transfer Queue").strong());
                    ui.label(
                        RichText::new(format!(
                            "{} active, {} completed",
                            active_count, completed_count
                        ))
                        .small()
                        .weak(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Clear completed").clicked() {
                            clear_completed = true;
                        }
                        if ui.small_button("Hide").clicked() {
                            self.transfers_open = false;
                        }
                    });
                });
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut count = 0;
                    for tab in &mut self.tabs {
                        for row in &mut tab.transfers {
                            count += 1;
                            let filename = row
                                .request
                                .remote
                                .rsplit('/')
                                .next()
                                .unwrap_or(&row.request.remote);
                            let file_type = FileType::from_filename(filename);

                            let item_rect = ui.available_rect_before_wrap();
                            ui.allocate_ui(Vec2::new(item_rect.width(), 32.0), |ui| {
                                ui.horizontal(|ui| {
                                    // File type icon
                                    let (icon_rect, _) = ui
                                        .allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                                    let texture = file_icon_texture(ui.ctx(), file_type, dark);
                                    let uv = Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    );
                                    ui.painter().image(
                                        texture.id(),
                                        icon_rect,
                                        uv,
                                        Color32::WHITE,
                                    );

                                    // File name + path
                                    ui.vertical(|ui| {
                                        ui.label(RichText::new(filename).strong().size(13.0));
                                        ui.label(
                                            RichText::new(format!(
                                                "{} → {}",
                                                tab.host.name, row.request.remote
                                            ))
                                            .small()
                                            .weak(),
                                        );
                                    });

                                    ui.add_space(16.0);

                                    // Progress bar
                                    let ratio = if row.total == 0 {
                                        if matches!(row.state, TransferState::Complete) {
                                            1.0
                                        } else {
                                            0.0
                                        }
                                    } else {
                                        (row.done as f32 / row.total as f32).clamp(0.0, 1.0)
                                    };
                                    let pct = (ratio * 100.0) as u32;

                                    ui.add(
                                        egui::ProgressBar::new(ratio)
                                            .desired_width(120.0)
                                            .desired_height(6.0),
                                    );

                                    ui.label(
                                        RichText::new(format!(
                                            "{} / {} ({}%)",
                                            bytes(row.done),
                                            bytes(row.total),
                                            pct
                                        ))
                                        .small(),
                                    );

                                    if matches!(row.state, TransferState::Running) && row.speed > 0.0 {
                                        ui.label(
                                            RichText::new(format!("{}/s", bytes(row.speed as u64)))
                                                .small()
                                                .weak(),
                                        );
                                    }

                                    // State badges / buttons
                                    match &row.state {
                                        TransferState::Complete => {
                                            ui.colored_label(colors::SUCCESS, "● Complete");
                                        }
                                        TransferState::Running | TransferState::Queued => {
                                            if ui_icon_button(
                                                ui,
                                                UiIcon::Cancel,
                                                "Cancel transfer",
                                                dark,
                                            )
                                            .clicked()
                                            {
                                                row.request.cancel.cancel();
                                            }
                                        }
                                        TransferState::Cancelled => {
                                            ui.label(RichText::new("Cancelled").small().weak());
                                            if tab.connected
                                                && ui_icon_button(
                                                    ui,
                                                    UiIcon::Retry,
                                                    "Retry",
                                                    dark,
                                                )
                                                .clicked()
                                            {
                                                retries.push((tab.id, row.request.clone()));
                                            }
                                        }
                                        TransferState::Failed(err) => {
                                            ui.colored_label(colors::DANGER, format!("Failed: {err}"));
                                            if tab.connected
                                                && ui_icon_button(
                                                    ui,
                                                    UiIcon::Retry,
                                                    "Retry",
                                                    dark,
                                                )
                                                .clicked()
                                            {
                                                retries.push((tab.id, row.request.clone()));
                                            }
                                        }
                                    }
                                });
                            });
                            ui.separator();
                        }
                    }
                    if count == 0 {
                        ui.label(
                            RichText::new(
                                "Uploads and downloads appear here. Terminal remains available during transfers.",
                            )
                            .weak(),
                        );
                    }
                });
            });

        if clear_completed {
            for tab in &mut self.tabs {
                tab.transfers
                    .retain(|t| !matches!(t.state, TransferState::Complete));
            }
        }

        for (id, mut request) in retries {
            self.next_id += 1;
            request.id = self.next_id;
            request.cancel = CancellationToken::new();
            self.queue_transfer(id, request);
        }
    }
}
