use crate::app::{App, Confirmation, FileDialog, FileDialogKind, TransferRow};
use egui::{Color32, RichText};
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
        egui::SidePanel::right("files").default_width(280.0).width_range(230.0..=600.0).show(ctx,|ui|{
            let Some(tab)=self.tabs.get_mut(self.active) else{return;};let id=tab.id;
            ui.horizontal(|ui|{ui.label(RichText::new("SFTP").small().strong());ui.label(RichText::new(&tab.host.name).small().weak());});
            ui.add_enabled_ui(tab.connected,|ui|{
                ui.horizontal(|ui|{
                    if ui.add_enabled(!tab.history.is_empty(),egui::Button::new("←")).on_hover_text("Back").clicked() && let Some(path)=tab.history.pop(){operation=Some((id,FileOperation::List(path)));}
                    if ui.button("↑").on_hover_text("Parent directory").clicked(){tab.history.push(tab.path.clone());operation=Some((id,FileOperation::List(format!("{}/..",tab.path))));}
                    if ui.button("Refresh").clicked(){operation=Some((id,FileOperation::List(tab.path.clone())));}
                    if tab.busy{ui.spinner();}
                });
                let response=ui.add(egui::TextEdit::singleline(&mut tab.path_input).desired_width(f32::INFINITY));
                if response.lost_focus() && ui.input(|i|i.key_pressed(egui::Key::Enter)){tab.history.push(tab.path.clone());operation=Some((id,FileOperation::List(tab.path_input.clone())));}
                ui.add(egui::TextEdit::singleline(&mut tab.filter).hint_text("Filter files…").desired_width(f32::INFINITY));
                ui.horizontal(|ui|{
                    upload=ui.button("Upload…").clicked();
                    if ui.button("+ File").clicked(){dialog=Some(FileDialog {tab:id,kind:FileDialogKind::CreateFile,name:String::new(),error:None});}
                    if ui.button("+ Folder").clicked(){dialog=Some(FileDialog {tab:id,kind:FileDialogKind::CreateDirectory,name:String::new(),error:None});}
                });
            });
            ui.separator();
            egui::ScrollArea::vertical().id_salt("remote_files").auto_shrink([false,false]).show(ui,|ui|{
                for entry in &tab.entries {
                    if (!self.settings.show_hidden && entry.name.starts_with('.')) || !entry.name.to_lowercase().contains(&tab.filter.to_lowercase()){continue;}
                    let selected=tab.selected.as_ref().is_some_and(|s|s.name==entry.name);
                    let response=ui.selectable_label(selected,format!("{}  {}",if entry.directory{"▸"}else if entry.symlink{"↗"}else{"·"},entry.name));
                    if response.clicked(){tab.selected=Some(entry.clone());}
                    let path=match remote_join(&tab.path,&entry.name){Ok(path)=>path,Err(_)=>continue};
                    if response.double_clicked() && (entry.directory||entry.symlink){tab.history.push(tab.path.clone());operation=Some((id,FileOperation::List(path.clone())));}
                    response.on_hover_text(format!("{}\nSize: {}\nModified: {} (Unix seconds)\nOwner: {}:{}\nPermissions: {:o}",path,bytes(entry.size),entry.modified.map(|v|v.to_string()).unwrap_or_else(||"unknown".into()),entry.uid.map(|v|v.to_string()).unwrap_or_else(||"?".into()),entry.gid.map(|v|v.to_string()).unwrap_or_else(||"?".into()),entry.permissions.unwrap_or(0)&0o7777)).context_menu(|ui|{
                        if entry.directory && ui.button("Open").clicked(){tab.history.push(tab.path.clone());operation=Some((id,FileOperation::List(path.clone())));ui.close();}
                        if !entry.directory && !entry.symlink && ui.button("Download…").clicked(){download=Some((id,path.clone(),entry.name.clone()));ui.close();}
                        if ui.button("Copy remote path").clicked(){ui.ctx().copy_text(path.clone());ui.close();}
                        if ui.button("Rename…").clicked(){dialog=Some(FileDialog {tab:id,kind:FileDialogKind::Rename(path.clone()),name:entry.name.clone(),error:None});ui.close();}
                        if ui.button("Permissions…").clicked(){dialog=Some(FileDialog {tab:id,kind:FileDialogKind::Permissions(path.clone()),name:format!("{:o}",entry.permissions.unwrap_or(0)&0o7777),error:None});ui.close();}
                        if ui.button("Delete…").clicked(){deletion=Some(Confirmation::File(id,FileOperation::Delete(path.clone(),entry.directory)));ui.close();}
                    });
                    ui.horizontal(|ui|{ui.add_space(18.0);ui.label(RichText::new(if entry.directory{"Directory".into()}else{bytes(entry.size)}).small().weak());});
                }
                if tab.entries.is_empty() && !tab.busy{ui.label(RichText::new("Directory empty or unavailable").weak());}
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
                    ui.colored_label(Color32::LIGHT_RED, error);
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
        egui::TopBottomPanel::bottom("transfers").resizable(true).default_height(180.0).height_range(100.0..=400.0).show(ctx,|ui|{
            ui.horizontal(|ui|{ui.label(RichText::new("TRANSFER QUEUE").small().strong());ui.label(RichText::new("Session history · streamed files").small().weak());if ui.small_button("Hide").clicked(){self.transfers_open=false;}});
            egui::ScrollArea::vertical().show(ui,|ui|{
                let mut count=0;
                for tab in &mut self.tabs {for row in &mut tab.transfers{count+=1;
                    ui.horizontal(|ui|{
                        ui.label(format!("{}  {}",match row.request.direction{Direction::Upload=>"↑",Direction::Download=>"↓"},tab.host.name));
                        ui.label(&row.request.remote);
                        let ratio=if row.total==0{0.0}else{row.done as f32/row.total as f32};ui.add(egui::ProgressBar::new(ratio).desired_width(140.0));
                        ui.label(format!("{} / {} · {}/s",bytes(row.done),bytes(row.total),bytes(row.speed.max(0.0) as u64)));
                        ui.label(match &row.state{TransferState::Queued=>"Queued",TransferState::Running=>"Transferring",TransferState::Complete=>"Complete",TransferState::Cancelled=>"Cancelled",TransferState::Failed(_)=>"Failed"});
                        if matches!(row.state,TransferState::Queued|TransferState::Running) && ui.small_button("Cancel").clicked(){row.request.cancel.cancel();}
                        if matches!(row.state,TransferState::Failed(_)|TransferState::Cancelled) && tab.connected && ui.small_button("Retry").clicked(){retries.push((tab.id,row.request.clone()));}
                    });
                    if let TransferState::Failed(error)=&row.state{ui.colored_label(Color32::LIGHT_RED,error);}
                }}
                if count==0{ui.label(RichText::new("Uploads and downloads appear here. Terminal remains available during transfers.").weak());}
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
