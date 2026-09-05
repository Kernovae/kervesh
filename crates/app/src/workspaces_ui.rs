use egui::{Align2, Color32, RichText, Vec2};
use kervesh_core::{Host, SessionWorkspace};

#[derive(Default)]
pub struct WorkspacesUi {
    pub editing: Option<SessionWorkspace>,
    pub filter: String,
    pub selected_tag: Option<String>,
    pub tag_input: String,
    pub error_message: Option<String>,
}

pub enum WorkspaceAction {
    ConnectAll(Vec<String>),
    Save(SessionWorkspace),
    Delete(String),
}

impl WorkspacesUi {
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        workspaces: &[SessionWorkspace],
        hosts: &[Host],
        is_open: &mut bool,
    ) -> Option<WorkspaceAction> {
        let mut action = None;
        if !*is_open {
            return None;
        }

        let mut modal_open = *is_open;
        egui::Window::new(RichText::new("Session Workspaces & Groups").strong())
            .open(&mut modal_open)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .resizable(true)
            .default_size(Vec2::new(720.0, 500.0))
            .show(ctx, |ui| {
                let mut dismiss_err = false;
                if let Some(err) = &self.error_message {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("⚠ {err}")).color(Color32::from_rgb(239, 68, 68)));
                            if ui.button("Dismiss").clicked() {
                                dismiss_err = true;
                            }
                        });
                    });
                    ui.add_space(4.0);
                }
                if dismiss_err {
                    self.error_message = None;
                }

                ui.horizontal(|ui| {
                    ui.label("Filter:");
                    ui.text_edit_singleline(&mut self.filter);

                    if ui.button(RichText::new("➕ New Workspace").color(Color32::from_rgb(74, 222, 128))).clicked() {
                        self.editing = Some(SessionWorkspace::new("New Workspace", ""));
                    }
                });

                ui.separator();

                let filtered: Vec<&SessionWorkspace> = workspaces
                    .iter()
                    .filter(|w| {
                        if self.filter.is_empty() {
                            return true;
                        }
                        let query = self.filter.to_lowercase();
                        w.name.to_lowercase().contains(&query)
                            || w.description.to_lowercase().contains(&query)
                            || w.tags.iter().any(|t| t.to_lowercase().contains(&query))
                    })
                    .collect();

                if filtered.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(32.0);
                        ui.label(RichText::new("No session workspaces created").italics().color(Color32::GRAY));
                        ui.label("Group hosts into named clusters (e.g. 'Production Stack', 'Dev DBs') for 1-click batch reconnect");
                    });
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(380.0)
                        .show(ui, |ui| {
                            for ws in filtered {
                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(RichText::new(&ws.name).strong());
                                                ui.label(
                                                    RichText::new(format!("({} hosts)", ws.host_ids.len()))
                                                        .size(11.0)
                                                        .color(Color32::LIGHT_GRAY),
                                                );
                                            });

                                            if !ws.description.is_empty() {
                                                ui.label(RichText::new(&ws.description).size(12.0).color(Color32::GRAY));
                                            }

                                            // Tags
                                            if !ws.tags.is_empty() {
                                                ui.horizontal(|ui| {
                                                    for tag in &ws.tags {
                                                        ui.label(
                                                            RichText::new(format!("#{tag}"))
                                                                .size(11.0)
                                                                .color(Color32::from_rgb(147, 197, 253)),
                                                        );
                                                    }
                                                });
                                            }

                                            // Host chips
                                            let host_names: Vec<&str> = ws
                                                .host_ids
                                                .iter()
                                                .filter_map(|hid| hosts.iter().find(|h| &h.id == hid).map(|h| h.name.as_str()))
                                                .collect();
                                            if !host_names.is_empty() {
                                                ui.label(
                                                    RichText::new(format!("Hosts: {}", host_names.join(", ")))
                                                        .size(11.0)
                                                        .color(Color32::LIGHT_GRAY),
                                                );
                                            }
                                        });

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.button(RichText::new("🚀 Connect All").color(Color32::from_rgb(34, 197, 94))).clicked() {
                                                action = Some(WorkspaceAction::ConnectAll(ws.host_ids.clone()));
                                            }

                                            if ui.button("✏ Edit").clicked() {
                                                self.editing = Some(ws.clone());
                                            }

                                            if ui.button(RichText::new("🗑").color(Color32::GRAY)).clicked() {
                                                action = Some(WorkspaceAction::Delete(ws.id.clone()));
                                            }
                                        });
                                    });
                                });
                                ui.add_space(2.0);
                            }
                        });
                }
            });

        *is_open = modal_open;

        // Edit Dialog
        if let Some(mut draft) = self.editing.take() {
            let mut keep_editing = true;
            egui::Window::new(if draft.id.is_empty() {
                "New Session Workspace"
            } else {
                "Edit Workspace"
            })
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .resizable(false)
            .default_size(Vec2::new(520.0, 420.0))
            .show(ctx, |ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut draft.name);

                ui.label("Description:");
                ui.text_edit_singleline(&mut draft.description);

                ui.add_space(4.0);
                ui.label("Tags (press Add or Enter):");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.tag_input);
                    if ui.button("Add Tag").clicked() && !self.tag_input.trim().is_empty() {
                        draft.add_tag(self.tag_input.trim());
                        self.tag_input.clear();
                    }
                });

                if !draft.tags.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        let mut remove_tag = None;
                        for tag in &draft.tags {
                            if ui.button(format!("#{tag} ✕")).clicked() {
                                remove_tag = Some(tag.clone());
                            }
                        }
                        if let Some(t) = remove_tag {
                            draft.tags.retain(|item| item != &t);
                        }
                    });
                }

                ui.add_space(6.0);
                ui.label("Included Hosts:");
                egui::ScrollArea::vertical()
                    .max_height(140.0)
                    .show(ui, |ui| {
                        for host in hosts {
                            let mut included = draft.host_ids.contains(&host.id);
                            if ui.checkbox(&mut included, &host.name).changed() {
                                if included {
                                    draft.add_host(&host.id);
                                } else {
                                    draft.remove_host(&host.id);
                                }
                            }
                        }
                    });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(
                            RichText::new("Save Workspace")
                                .strong()
                                .color(Color32::from_rgb(74, 222, 128)),
                        )
                        .clicked()
                    {
                        if let Err(e) = draft.validate() {
                            self.error_message = Some(e.to_string());
                        } else {
                            action = Some(WorkspaceAction::Save(draft.clone()));
                            keep_editing = false;
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        keep_editing = false;
                    }
                });
            });

            if keep_editing {
                self.editing = Some(draft);
            }
        }

        action
    }
}
