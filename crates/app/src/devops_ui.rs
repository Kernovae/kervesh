use egui::{Align, Color32, Layout, RichText, ScrollArea, Stroke, TextEdit, Vec2};
use kervesh_core::devops::{
    DockerAction, DockerContainer, DockerImage, NetDiagResult, NetDiagTool, SystemdAction,
    SystemdUnit,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DevOpsTab {
    Docker,
    Systemd,
    NetworkDiag,
}

#[derive(Debug, Clone)]
pub struct DevOpsUiState {
    pub open: bool,
    pub active_tab: DevOpsTab,
    // Docker state
    pub containers: Vec<DockerContainer>,
    pub images: Vec<DockerImage>,
    pub docker_filter: String,
    pub docker_logs_id: Option<String>,
    pub docker_logs: Option<String>,
    // Systemd state
    pub units: Vec<SystemdUnit>,
    pub systemd_filter: String,
    pub systemd_logs_unit: Option<String>,
    pub systemd_logs: Option<String>,
    // NetDiag state
    pub diag_tool: NetDiagTool,
    pub diag_target: String,
    pub diag_extra: String,
    pub diag_running: bool,
    pub diag_results: Vec<NetDiagResult>,
    pub error: Option<String>,
}

impl Default for DevOpsUiState {
    fn default() -> Self {
        Self {
            open: false,
            active_tab: DevOpsTab::Docker,
            containers: Vec::new(),
            images: Vec::new(),
            docker_filter: String::new(),
            docker_logs_id: None,
            docker_logs: None,
            units: Vec::new(),
            systemd_filter: String::new(),
            systemd_logs_unit: None,
            systemd_logs: None,
            diag_tool: NetDiagTool::Ping,
            diag_target: String::new(),
            diag_extra: String::new(),
            diag_running: false,
            diag_results: Vec::new(),
            error: None,
        }
    }
}

pub enum DevOpsUiAction {
    RefreshDocker,
    DockerAction(String, DockerAction),
    DockerLogs(String),
    RefreshSystemd,
    SystemdAction(String, SystemdAction),
    SystemdLogs(String),
    RunNetDiag {
        tool: NetDiagTool,
        target: String,
        port_or_type: Option<String>,
    },
    Close,
}

impl DevOpsUiState {
    pub fn show(&mut self, ctx: &egui::Context) -> Option<DevOpsUiAction> {
        if !self.open {
            return None;
        }

        let mut action = None;
        let mut open_flag = self.open;

        egui::Window::new("Sysadmin & DevOps Toolbox")
            .open(&mut open_flag)
            .default_size(Vec2::new(880.0, 580.0))
            .min_size(Vec2::new(600.0, 400.0))
            .resizable(true)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(
                            self.active_tab == DevOpsTab::Docker,
                            " 🐳 Docker & Containers ",
                        )
                        .clicked()
                    {
                        self.active_tab = DevOpsTab::Docker;
                        if self.containers.is_empty() {
                            action = Some(DevOpsUiAction::RefreshDocker);
                        }
                    }
                    if ui
                        .selectable_label(
                            self.active_tab == DevOpsTab::Systemd,
                            " ⚙ Systemd Services ",
                        )
                        .clicked()
                    {
                        self.active_tab = DevOpsTab::Systemd;
                        if self.units.is_empty() {
                            action = Some(DevOpsUiAction::RefreshSystemd);
                        }
                    }
                    if ui
                        .selectable_label(
                            self.active_tab == DevOpsTab::NetworkDiag,
                            " 🌐 Network Diagnostics ",
                        )
                        .clicked()
                    {
                        self.active_tab = DevOpsTab::NetworkDiag;
                    }
                });

                ui.separator();

                if let Some(err) = &self.error {
                    ui.colored_label(Color32::from_rgb(235, 87, 87), format!("Error: {err}"));
                    ui.add_space(4.0);
                }

                match self.active_tab {
                    DevOpsTab::Docker => {
                        self.render_docker_tab(ui, &mut action);
                    }
                    DevOpsTab::Systemd => {
                        self.render_systemd_tab(ui, &mut action);
                    }
                    DevOpsTab::NetworkDiag => {
                        self.render_netdiag_tab(ui, &mut action);
                    }
                }

                ui.add_space(8.0);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Close").clicked() {
                        action = Some(DevOpsUiAction::Close);
                    }
                });
            });

        if !open_flag {
            self.open = false;
            if action.is_none() {
                action = Some(DevOpsUiAction::Close);
            }
        }

        action
    }

    fn render_docker_tab(&mut self, ui: &mut egui::Ui, action: &mut Option<DevOpsUiAction>) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Filter:").strong());
            ui.add(
                TextEdit::singleline(&mut self.docker_filter)
                    .hint_text("container name, image, ID…")
                    .desired_width(200.0),
            );
            if ui.button(" 🔄 Refresh Containers ").clicked() {
                *action = Some(DevOpsUiAction::RefreshDocker);
            }
        });

        ui.add_space(4.0);

        let mut close_docker_logs = false;
        if let Some(logs) = &self.docker_logs {
            let id = self.docker_logs_id.as_deref().unwrap_or("Container");
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Logs: {}", id)).strong());
                    if ui.button("Close Logs").clicked() {
                        close_docker_logs = true;
                    }
                });
                ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                    ui.label(RichText::new(logs).monospace().size(11.0));
                });
            });
            ui.add_space(4.0);
        }
        if close_docker_logs {
            self.docker_logs = None;
            self.docker_logs_id = None;
        }

        ui.label(RichText::new(format!("Containers ({})", self.containers.len())).strong());
        ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            let query = self.docker_filter.to_lowercase();
            let filtered: Vec<_> = self
                .containers
                .iter()
                .filter(|c| {
                    query.is_empty()
                        || c.names.to_lowercase().contains(&query)
                        || c.image.to_lowercase().contains(&query)
                        || c.id.to_lowercase().contains(&query)
                })
                .collect();

            if filtered.is_empty() {
                ui.label(
                    RichText::new("No containers found. Docker might not be running or installed.")
                        .weak()
                        .italics(),
                );
            }

            for c in filtered {
                let frame = egui::Frame::group(ui.style())
                    .fill(ui.visuals().window_fill().gamma_multiply(0.4))
                    .stroke(Stroke::new(
                        1.0_f32,
                        ui.visuals().weak_text_color().gamma_multiply(0.15),
                    ))
                    .inner_margin(4.0);

                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (status_icon, status_color) = if c.running {
                            ("● UP", Color32::from_rgb(60, 210, 120))
                        } else {
                            ("○ STOPPED", Color32::from_rgb(180, 180, 180))
                        };

                        ui.label(
                            RichText::new(status_icon)
                                .color(status_color)
                                .strong()
                                .size(11.0),
                        );
                        ui.label(RichText::new(&c.names).strong());
                        ui.label(RichText::new(format!("({})", c.image)).weak().size(11.0));

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.small_button("Logs").clicked() {
                                *action = Some(DevOpsUiAction::DockerLogs(c.id.clone()));
                            }
                            if c.running {
                                if ui.small_button("Restart").clicked() {
                                    *action = Some(DevOpsUiAction::DockerAction(
                                        c.id.clone(),
                                        DockerAction::Restart,
                                    ));
                                }
                                if ui.small_button("Stop").clicked() {
                                    *action = Some(DevOpsUiAction::DockerAction(
                                        c.id.clone(),
                                        DockerAction::Stop,
                                    ));
                                }
                            } else if ui.small_button("Start").clicked() {
                                *action = Some(DevOpsUiAction::DockerAction(
                                    c.id.clone(),
                                    DockerAction::Start,
                                ));
                            }
                        });
                    });
                    if !c.ports.is_empty() {
                        ui.label(
                            RichText::new(format!("Ports: {}", c.ports))
                                .weak()
                                .size(10.5),
                        );
                    }
                });
                ui.add_space(2.0);
            }
        });
    }

    fn render_systemd_tab(&mut self, ui: &mut egui::Ui, action: &mut Option<DevOpsUiAction>) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Filter:").strong());
            ui.add(
                TextEdit::singleline(&mut self.systemd_filter)
                    .hint_text("service name, description…")
                    .desired_width(200.0),
            );
            if ui.button(" 🔄 Refresh Services ").clicked() {
                *action = Some(DevOpsUiAction::RefreshSystemd);
            }
        });

        ui.add_space(4.0);

        let mut close_systemd_logs = false;
        if let Some(logs) = &self.systemd_logs {
            let unit = self.systemd_logs_unit.as_deref().unwrap_or("Unit");
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Journal: {}", unit)).strong());
                    if ui.button("Close Journal").clicked() {
                        close_systemd_logs = true;
                    }
                });
                ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                    ui.label(RichText::new(logs).monospace().size(11.0));
                });
            });
            ui.add_space(4.0);
        }
        if close_systemd_logs {
            self.systemd_logs = None;
            self.systemd_logs_unit = None;
        }

        ui.label(RichText::new(format!("Systemd Services ({})", self.units.len())).strong());
        ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            let query = self.systemd_filter.to_lowercase();
            let filtered: Vec<_> = self
                .units
                .iter()
                .filter(|u| {
                    query.is_empty()
                        || u.name.to_lowercase().contains(&query)
                        || u.description.to_lowercase().contains(&query)
                })
                .collect();

            if filtered.is_empty() {
                ui.label(RichText::new("No systemd units found.").weak().italics());
            }

            for u in filtered {
                let frame = egui::Frame::group(ui.style())
                    .fill(ui.visuals().window_fill().gamma_multiply(0.4))
                    .stroke(Stroke::new(
                        1.0_f32,
                        ui.visuals().weak_text_color().gamma_multiply(0.15),
                    ))
                    .inner_margin(4.0);

                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (badge, color) = if u.is_active() {
                            ("ACTIVE", Color32::from_rgb(60, 210, 120))
                        } else if u.is_failed() {
                            ("FAILED", Color32::from_rgb(235, 87, 87))
                        } else {
                            ("INACTIVE", Color32::from_rgb(180, 180, 180))
                        };

                        ui.label(RichText::new(badge).color(color).strong().size(10.5));
                        ui.label(RichText::new(&u.name).strong());

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.small_button("Journal").clicked() {
                                *action = Some(DevOpsUiAction::SystemdLogs(u.name.clone()));
                            }
                            if u.is_active() {
                                if ui.small_button("Restart").clicked() {
                                    *action = Some(DevOpsUiAction::SystemdAction(
                                        u.name.clone(),
                                        SystemdAction::Restart,
                                    ));
                                }
                                if ui.small_button("Stop").clicked() {
                                    *action = Some(DevOpsUiAction::SystemdAction(
                                        u.name.clone(),
                                        SystemdAction::Stop,
                                    ));
                                }
                            } else if ui.small_button("Start").clicked() {
                                *action = Some(DevOpsUiAction::SystemdAction(
                                    u.name.clone(),
                                    SystemdAction::Start,
                                ));
                            }
                        });
                    });
                    if !u.description.is_empty() {
                        ui.label(RichText::new(&u.description).weak().size(11.0));
                    }
                });
                ui.add_space(2.0);
            }
        });
    }

    fn render_netdiag_tab(&mut self, ui: &mut egui::Ui, action: &mut Option<DevOpsUiAction>) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Tool:").strong());
            egui::ComboBox::from_id_salt("netdiag_tool_select")
                .selected_text(self.diag_tool.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.diag_tool, NetDiagTool::Ping, "Ping (ICMP)");
                    ui.selectable_value(
                        &mut self.diag_tool,
                        NetDiagTool::Traceroute,
                        "Traceroute (Hops)",
                    );
                    ui.selectable_value(
                        &mut self.diag_tool,
                        NetDiagTool::PortScan,
                        "Port Scan (NC)",
                    );
                    ui.selectable_value(
                        &mut self.diag_tool,
                        NetDiagTool::DnsLookup,
                        "DNS Lookup (Dig)",
                    );
                });

            ui.add_space(8.0);
            ui.label(RichText::new("Target:").strong());
            ui.add(
                TextEdit::singleline(&mut self.diag_target)
                    .hint_text("hostname or IP address")
                    .desired_width(180.0),
            );

            if self.diag_tool == NetDiagTool::PortScan {
                ui.label("Ports:");
                ui.add(
                    TextEdit::singleline(&mut self.diag_extra)
                        .hint_text("22,80,443")
                        .desired_width(100.0),
                );
            } else if self.diag_tool == NetDiagTool::DnsLookup {
                ui.label("Record:");
                ui.add(
                    TextEdit::singleline(&mut self.diag_extra)
                        .hint_text("A, AAAA, MX")
                        .desired_width(60.0),
                );
            }

            let run_btn = ui.button(RichText::new(" Run Diagnostic ").strong());
            if run_btn.clicked() && !self.diag_target.trim().is_empty() && !self.diag_running {
                self.diag_running = true;
                *action = Some(DevOpsUiAction::RunNetDiag {
                    tool: self.diag_tool,
                    target: self.diag_target.trim().to_string(),
                    port_or_type: if self.diag_extra.trim().is_empty() {
                        None
                    } else {
                        Some(self.diag_extra.trim().to_string())
                    },
                });
            }
        });

        ui.add_space(6.0);

        if self.diag_running {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Executing network diagnostic probe on remote host...");
            });
        }

        ui.separator();
        ui.label(RichText::new("Diagnostic Output History:").strong());

        ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            if self.diag_results.is_empty() && !self.diag_running {
                ui.label(
                    RichText::new(
                        "No network diagnostic outputs yet. Enter a target and click Run.",
                    )
                    .weak()
                    .italics(),
                );
            }

            for res in self.diag_results.iter().rev() {
                let frame = egui::Frame::group(ui.style())
                    .fill(ui.visuals().window_fill().gamma_multiply(0.4))
                    .stroke(Stroke::new(
                        1.0_f32,
                        ui.visuals().weak_text_color().gamma_multiply(0.2),
                    ))
                    .inner_margin(4.0);

                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("[{}] {}", res.tool.label(), res.target))
                                .strong(),
                        );
                    });
                    ui.add_space(2.0);
                    ui.label(RichText::new(&res.raw_output).monospace().size(11.0));
                });
                ui.add_space(4.0);
            }
        });
    }
}
