use crate::{
    app::{App, Confirmation, FileDialog, FileDialogKind, TransferRow},
    classify::FileType,
    icons::paint_file_icon,
    theme::colors,
};
use egui::{Pos2, Rect, RichText, Vec2};
use kervesh_core::bytes;
use kervesh_ssh::{
    CancellationToken, Command, Direction, FileOperation, TransferRequest, TransferState,
    remote_join,
};
use std::path::PathBuf;

impl App {
    pub(crate) fn file_sidebar(&mut self, ctx: &egui::Context) {
        let mut operation = None;
        let mut dialog = None;
        let mut deletion = None;
        let mut upload = false;
        let mut download = None;

        egui::SidePanel::right("files")
            .default_width(300.0)
            .width_range(240.0..=600.0)
            .show(ctx, |ui| {
                let Some(tab) = self.tabs.get_mut(self.active) else {
                    return;
                };
                let id = tab.id;

                ui.horizontal(|ui| {
                    ui.label(RichText::new("SFTP").small().strong());
                    ui.label(RichText::new(&tab.host.name).small().weak());
                    if tab.busy {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spinner();
                        });
                    }
                });

                ui.add_enabled_ui(tab.connected, |ui| {
                    // Action toolbar
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(!tab.history.is_empty(), egui::Button::new("←"))
                            .on_hover_text("Back")
                            .clicked()
                            && let Some(path) = tab.history.pop()
                        {
                            operation = Some((id, FileOperation::List(path)));
                        }
                        if ui.button("↑").on_hover_text("Parent directory").clicked() {
                            tab.history.push(tab.path.clone());
                            operation = Some((id, FileOperation::List(format!("{}/..", tab.path))));
                        }
                        if ui.button("⟳").on_hover_text("Refresh").clicked() {
                            operation = Some((id, FileOperation::List(tab.path.clone())));
                        }
                        if ui.button("⤒").on_hover_text("Upload local file…").clicked() {
                            upload = true;
                        }
                        if ui.button("+ File").on_hover_text("New file").clicked() {
                            dialog = Some(FileDialog {
                                tab: id,
                                kind: FileDialogKind::CreateFile,
                                name: String::new(),
                                error: None,
                            });
                        }
                        if ui.button("+ Folder").on_hover_text("New folder").clicked() {
                            dialog = Some(FileDialog {
                                tab: id,
                                kind: FileDialogKind::CreateDirectory,
                                name: String::new(),
                                error: None,
                            });
                        }
                    });

                    // Direct path edit / navigation
                    let path_resp = ui.add(
                        egui::TextEdit::singleline(&mut tab.path_input)
                            .hint_text("Remote path…")
                            .desired_width(f32::INFINITY),
                    );
                    if path_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
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

                // File rows
                let dark = self.settings.dark;
                let text_color = if dark {
                    colors::FOREGROUND
                } else {
                    colors::LIGHT_FOREGROUND
                };
                let icon_color = if dark {
                    colors::FOREGROUND
                } else {
                    colors::LIGHT_FOREGROUND
                };

                egui::ScrollArea::vertical()
                    .id_salt("remote_files")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
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
                            let response = ui.allocate_rect(item_rect, egui::Sense::click());

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
                            paint_file_icon(ui.painter(), icon_rect, file_type, icon_color);

                            // Render text clipped
                            let label_rect = Rect::from_min_max(
                                Pos2::new(item_rect.min.x + 28.0, item_rect.min.y + 3.0),
                                Pos2::new(item_rect.max.x - 6.0, item_rect.max.y - 3.0),
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

                            if response.double_clicked() && (entry.directory || entry.symlink) {
                                tab.history.push(tab.path.clone());
                                operation = Some((id, FileOperation::List(path.clone())));
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
                                            .map(|v| format!("{v} (Unix seconds)"))
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
                                    if !entry.directory
                                        && !entry.symlink
                                        && ui.button("Download…").clicked()
                                    {
                                        download = Some((id, path.clone(), entry.name.clone()));
                                        ui.close();
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
                                    ui.separator();
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

                        if tab.entries.is_empty() && !tab.busy {
                            ui.label(RichText::new("Directory empty or unavailable").weak());
                        }
                    });
            });

        if let Some((id, operation)) = operation {
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
        egui::TopBottomPanel::bottom("transfers")
            .resizable(true)
            .default_height(180.0)
            .height_range(100.0..=400.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("TRANSFER QUEUE").small().strong());
                    ui.label(RichText::new("Session history · streamed files").small().weak());
                    if ui.small_button("Hide").clicked() {
                        self.transfers_open = false;
                    }
                });
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut count = 0;
                    for tab in &mut self.tabs {
                        for row in &mut tab.transfers {
                            count += 1;
                            ui.horizontal(|ui| {
                                ui.label(format!(
                                    "{}  {}",
                                    match row.request.direction {
                                        Direction::Upload => "↑",
                                        Direction::Download => "↓",
                                    },
                                    tab.host.name
                                ));
                                ui.label(&row.request.remote);
                                let ratio = if row.total == 0 {
                                    0.0
                                } else {
                                    row.done as f32 / row.total as f32
                                };
                                ui.add(egui::ProgressBar::new(ratio).desired_width(140.0));
                                ui.label(format!(
                                    "{} / {} · {}/s",
                                    bytes(row.done),
                                    bytes(row.total),
                                    bytes(row.speed.max(0.0) as u64)
                                ));
                                ui.label(match &row.state {
                                    TransferState::Queued => "Queued",
                                    TransferState::Running => "Transferring",
                                    TransferState::Complete => "Complete",
                                    TransferState::Cancelled => "Cancelled",
                                    TransferState::Failed(_) => "Failed",
                                });
                                if matches!(
                                    row.state,
                                    TransferState::Queued | TransferState::Running
                                ) && ui.small_button("Cancel").clicked()
                                {
                                    row.request.cancel.cancel();
                                }
                                if matches!(
                                    row.state,
                                    TransferState::Failed(_) | TransferState::Cancelled
                                ) && tab.connected
                                    && ui.small_button("Retry").clicked()
                                {
                                    retries.push((tab.id, row.request.clone()));
                                }
                            });
                            if let TransferState::Failed(error) = &row.state {
                                ui.colored_label(colors::DANGER, error);
                            }
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
        for (id, mut request) in retries {
            self.next_id += 1;
            request.id = self.next_id;
            request.cancel = CancellationToken::new();
            self.queue_transfer(id, request);
        }
    }
}
