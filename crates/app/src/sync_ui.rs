use egui::{Color32, RichText, ScrollArea, Stroke, TextEdit, Vec2};
use kervesh_core::bytes;
use kervesh_core::sync::{SyncActionKind, SyncConflictPolicy, SyncDirection, SyncPlan};

#[derive(Debug, Clone)]
pub struct SyncUiState {
    pub open: bool,
    pub local_dir: String,
    pub remote_dir: String,
    pub direction: SyncDirection,
    pub policy: SyncConflictPolicy,
    pub computing: bool,
    pub executing: bool,
    pub plan: Option<SyncPlan>,
    pub error: Option<String>,
    pub success_message: Option<String>,
}

impl Default for SyncUiState {
    fn default() -> Self {
        Self {
            open: false,
            local_dir: std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".into()),
            remote_dir: "/".into(),
            direction: SyncDirection::LocalToRemote,
            policy: SyncConflictPolicy::NewerWins,
            computing: false,
            executing: false,
            plan: None,
            error: None,
            success_message: None,
        }
    }
}

pub enum SyncUiAction {
    ComputePlan {
        local_dir: String,
        remote_dir: String,
        direction: SyncDirection,
        policy: SyncConflictPolicy,
    },
    ExecuteSync(SyncPlan),
    Close,
}

impl SyncUiState {
    pub fn open_for_remote(&mut self, remote_dir: impl Into<String>) {
        let r = remote_dir.into();
        self.remote_dir = if r.is_empty() { "/".into() } else { r };
        self.open = true;
        self.error = None;
        self.success_message = None;
        self.plan = None;
    }

    pub fn set_plan(&mut self, plan: SyncPlan) {
        self.computing = false;
        self.plan = Some(plan);
        self.error = None;
    }

    pub fn set_error(&mut self, err: String) {
        self.computing = false;
        self.executing = false;
        self.error = Some(err);
    }

    pub fn set_complete(&mut self, msg: String) {
        self.executing = false;
        self.success_message = Some(msg);
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<SyncUiAction> {
        if !self.open {
            return None;
        }

        let mut action = None;
        let mut open_flag = self.open;

        egui::Window::new("Directory Synchronization")
            .open(&mut open_flag)
            .default_size(Vec2::new(820.0, 560.0))
            .min_size(Vec2::new(550.0, 380.0))
            .resizable(true)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Local Directory:").strong());
                    ui.add(TextEdit::singleline(&mut self.local_dir).desired_width(ui.available_width() - 30.0));
                });

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Remote Directory:").strong());
                    ui.add(TextEdit::singleline(&mut self.remote_dir).desired_width(ui.available_width() - 30.0));
                });

                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Direction:").strong());
                    egui::ComboBox::from_id_salt("sync_dir_select")
                        .selected_text(match self.direction {
                            SyncDirection::LocalToRemote => "Local → Remote (Upload)",
                            SyncDirection::RemoteToLocal => "Remote → Local (Download)",
                            SyncDirection::BiDirectional => "Two-Way Bidirectional",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.direction, SyncDirection::LocalToRemote, "Local → Remote (Upload)");
                            ui.selectable_value(&mut self.direction, SyncDirection::RemoteToLocal, "Remote → Local (Download)");
                            ui.selectable_value(&mut self.direction, SyncDirection::BiDirectional, "Two-Way Bidirectional");
                        });

                    ui.add_space(16.0);

                    ui.label(RichText::new("Conflict Resolution:").strong());
                    egui::ComboBox::from_id_salt("sync_conflict_select")
                        .selected_text(match self.policy {
                            SyncConflictPolicy::NewerWins => "Newer Timestamp Wins",
                            SyncConflictPolicy::Overwrite => "Always Overwrite Destination",
                            SyncConflictPolicy::Skip => "Skip Conflicted Files",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.policy, SyncConflictPolicy::NewerWins, "Newer Timestamp Wins");
                            ui.selectable_value(&mut self.policy, SyncConflictPolicy::Overwrite, "Always Overwrite Destination");
                            ui.selectable_value(&mut self.policy, SyncConflictPolicy::Skip, "Skip Conflicted Files");
                        });
                });

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    let preview_btn = ui.button(RichText::new(" Compare & Preview Plan ").strong());
                    if preview_btn.clicked() && !self.computing && !self.executing {
                        self.computing = true;
                        self.plan = None;
                        self.error = None;
                        self.success_message = None;
                        action = Some(SyncUiAction::ComputePlan {
                            local_dir: self.local_dir.clone(),
                            remote_dir: self.remote_dir.clone(),
                            direction: self.direction,
                            policy: self.policy,
                        });
                    }

                    if let Some(plan) = &self.plan {
                        let active_items = plan.items.iter().filter(|i| !matches!(i.action, SyncActionKind::Identical | SyncActionKind::Conflict)).count();
                        let sync_btn = ui.add_enabled(
                            !self.computing && !self.executing && active_items > 0,
                            egui::Button::new(RichText::new(format!(" Execute Sync ({} actions, {}) ", active_items, bytes(plan.total_bytes))).color(Color32::from_rgb(60, 210, 120)).strong()),
                        );
                        if sync_btn.clicked() {
                            self.executing = true;
                            self.error = None;
                            action = Some(SyncUiAction::ExecuteSync(plan.clone()));
                        }
                    }
                });

                ui.separator();

                if let Some(err) = &self.error {
                    ui.colored_label(Color32::from_rgb(235, 87, 87), format!("Error: {err}"));
                    ui.add_space(4.0);
                }

                if let Some(msg) = &self.success_message {
                    ui.colored_label(Color32::from_rgb(60, 210, 120), format!("✓ {msg}"));
                    ui.add_space(4.0);
                }

                if self.computing {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Scanning local and remote directories...");
                    });
                } else if self.executing {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new("Executing sync transfers in background...").strong());
                    });
                }

                if let Some(plan) = &self.plan {
                    let mut uploads = 0;
                    let mut downloads = 0;
                    let mut deletes = 0;
                    let mut conflicts = 0;
                    let mut identical = 0;

                    for item in &plan.items {
                        match item.action {
                            SyncActionKind::Upload => uploads += 1,
                            SyncActionKind::Download => downloads += 1,
                            SyncActionKind::DeleteLocal | SyncActionKind::DeleteRemote => deletes += 1,
                            SyncActionKind::Conflict => conflicts += 1,
                            SyncActionKind::Identical => identical += 1,
                        }
                    }

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("Plan Summary: ↑ {} Uploads, ↓ {} Downloads, ✕ {} Deletions, ⚠ {} Conflicts, ‒ {} Identical (Total {})", uploads, downloads, deletes, conflicts, identical, bytes(plan.total_bytes))).strong().size(12.0));
                    });

                    ui.add_space(4.0);

                    // Diff Table
                    ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                        for item in &plan.items {
                            let (badge_text, badge_color) = match item.action {
                                SyncActionKind::Upload => ("↑ UPLOAD", Color32::from_rgb(70, 170, 255)),
                                SyncActionKind::Download => ("↓ DOWNLOAD", Color32::from_rgb(80, 220, 120)),
                                SyncActionKind::DeleteLocal => ("✕ DEL LOCAL", Color32::from_rgb(235, 90, 90)),
                                SyncActionKind::DeleteRemote => ("✕ DEL REMOTE", Color32::from_rgb(235, 90, 90)),
                                SyncActionKind::Conflict => ("⚠ CONFLICT", Color32::from_rgb(255, 180, 50)),
                                SyncActionKind::Identical => ("- IDENTICAL", Color32::from_rgb(130, 130, 130)),
                            };

                            let frame = egui::Frame::group(ui.style())
                                .fill(ui.visuals().window_fill().gamma_multiply(0.4))
                                .stroke(Stroke::new(1.0_f32, ui.visuals().weak_text_color().gamma_multiply(0.15)))
                                .inner_margin(3.0);

                            frame.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("[{:^12}]", badge_text)).color(badge_color).monospace().strong());
                                    ui.label(RichText::new(&item.rel_path).strong());
                                });
                            });
                            ui.add_space(1.5);
                        }
                    });
                } else if !self.computing {
                    ui.vertical_centered(|ui| {
                        ui.add_space(30.0);
                        ui.label(RichText::new("Click 'Compare & Preview Plan' to inspect directory differences.").weak().italics());
                    });
                }

                ui.add_space(8.0);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Close").clicked() {
                        action = Some(SyncUiAction::Close);
                    }
                });
            });

        if !open_flag {
            self.open = false;
            if action.is_none() {
                action = Some(SyncUiAction::Close);
            }
        }

        action
    }
}
