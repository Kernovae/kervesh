use egui::{Align2, Color32, RichText, Vec2};
use kervesh_core::{Host, TunnelConfig, TunnelKind, TunnelStats};
use std::collections::HashMap;

#[derive(Default)]
pub struct TunnelsUi {
    pub editing: Option<TunnelConfig>,
    pub filter: String,
    pub error_message: Option<String>,
}

pub enum TunnelAction {
    Start(TunnelConfig),
    Stop(String),
    Save(TunnelConfig),
    Delete(String),
}

impl TunnelsUi {
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        tunnels: &[TunnelConfig],
        hosts: &[Host],
        active_tunnels: &HashMap<String, TunnelStats>,
        is_open: &mut bool,
    ) -> Option<TunnelAction> {
        let mut action = None;
        if !*is_open {
            return None;
        }

        let mut modal_open = *is_open;
        egui::Window::new(RichText::new("SSH Tunnel & Proxy Manager").strong())
            .open(&mut modal_open)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .resizable(true)
            .default_size(Vec2::new(760.0, 520.0))
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

                    if ui.button(RichText::new("➕ New Tunnel").color(Color32::from_rgb(74, 222, 128))).clicked() {
                        let default_host = hosts.first().map(|h| h.id.clone()).unwrap_or_default();
                        self.editing = Some(TunnelConfig::new(
                            default_host,
                            "New Tunnel",
                            TunnelKind::Local,
                            8080,
                            "127.0.0.1",
                            80,
                        ));
                    }
                });

                ui.separator();

                let filtered: Vec<&TunnelConfig> = tunnels
                    .iter()
                    .filter(|t| {
                        if self.filter.is_empty() {
                            return true;
                        }
                        let query = self.filter.to_lowercase();
                        t.name.to_lowercase().contains(&query)
                            || t.bind_addr.to_lowercase().contains(&query)
                            || t.target_host.to_lowercase().contains(&query)
                            || t.kind.as_str().to_lowercase().contains(&query)
                    })
                    .collect();

                if filtered.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(32.0);
                        ui.label(RichText::new("No configured SSH tunnels").italics().color(Color32::GRAY));
                        ui.label("Forward local ports (-L), expose remote ports (-R), or use dynamic SOCKS5 (-D)");
                    });
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(380.0)
                        .show(ui, |ui| {
                            for tunnel in filtered {
                                let is_active = active_tunnels.contains_key(&tunnel.id);
                                let stats = active_tunnels.get(&tunnel.id);

                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        // Status dot
                                        let (status_color, _status_text) = if is_active {
                                            (Color32::from_rgb(34, 197, 94), "ACTIVE")
                                        } else {
                                            (Color32::from_rgb(107, 114, 128), "STOPPED")
                                        };
                                        ui.painter().circle_filled(
                                            ui.cursor().min + Vec2::new(6.0, 10.0),
                                            4.0,
                                            status_color,
                                        );
                                        ui.add_space(14.0);

                                        ui.vertical(|ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(RichText::new(&tunnel.name).strong());
                                                ui.label(
                                                    RichText::new(format!("[{}]", tunnel.kind.as_str()))
                                                        .size(11.0)
                                                        .color(Color32::from_rgb(147, 197, 253)),
                                                );
                                                if tunnel.auto_start {
                                                    ui.label(
                                                        RichText::new("⚡ Auto-Start")
                                                            .size(10.0)
                                                            .color(Color32::from_rgb(250, 204, 21)),
                                                    );
                                                }
                                            });

                                            let host_label = hosts
                                                .iter()
                                                .find(|h| h.id == tunnel.host_id)
                                                .map(|h| h.name.as_str())
                                                .unwrap_or("Unknown Host");

                                            ui.label(
                                                RichText::new(format!(
                                                    "Host: {}  |  {}",
                                                    host_label,
                                                    tunnel.display_summary()
                                                ))
                                                .size(12.0)
                                                .color(Color32::LIGHT_GRAY),
                                            );

                                            if let Some(st) = stats {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "Conns: {} | Rx: {} | Tx: {}",
                                                            st.active_connections,
                                                            format_bytes(st.bytes_rx),
                                                            format_bytes(st.bytes_tx)
                                                        ))
                                                        .size(11.0)
                                                        .color(Color32::from_rgb(134, 239, 172)),
                                                    );
                                                });
                                            }
                                        });

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if is_active {
                                                if ui.button(RichText::new("⏹ Stop").color(Color32::from_rgb(239, 68, 68))).clicked() {
                                                    action = Some(TunnelAction::Stop(tunnel.id.clone()));
                                                }
                                            } else {
                                                if ui.button(RichText::new("▶ Start").color(Color32::from_rgb(34, 197, 94))).clicked() {
                                                    action = Some(TunnelAction::Start(tunnel.clone()));
                                                }
                                            }

                                            if ui.button("✏ Edit").clicked() {
                                                self.editing = Some(tunnel.clone());
                                            }

                                            if !is_active && ui.button(RichText::new("🗑").color(Color32::GRAY)).clicked() {
                                                action = Some(TunnelAction::Delete(tunnel.id.clone()));
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

        // Edit / Create Dialog
        if let Some(mut draft) = self.editing.take() {
            let mut keep_editing = true;
            egui::Window::new(if draft.id.is_empty() {
                "New SSH Tunnel"
            } else {
                "Edit SSH Tunnel"
            })
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .resizable(false)
            .default_size(Vec2::new(480.0, 380.0))
            .show(ctx, |ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut draft.name);

                ui.label("Target Host Profile:");
                egui::ComboBox::from_id_salt("tunnel_host_select")
                    .selected_text(
                        hosts
                            .iter()
                            .find(|h| h.id == draft.host_id)
                            .map(|h| h.name.as_str())
                            .unwrap_or("Select Host"),
                    )
                    .show_ui(ui, |ui| {
                        for host in hosts {
                            ui.selectable_value(&mut draft.host_id, host.id.clone(), &host.name);
                        }
                    });

                ui.label("Tunnel Type:");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut draft.kind, TunnelKind::Local, "Local (-L)");
                    ui.selectable_value(&mut draft.kind, TunnelKind::Remote, "Remote (-R)");
                    ui.selectable_value(
                        &mut draft.kind,
                        TunnelKind::Dynamic,
                        "Dynamic SOCKS5 (-D)",
                    );
                });

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Bind Address:");
                    ui.text_edit_singleline(&mut draft.bind_addr);
                    ui.label("Port:");
                    let mut port_str = draft.bind_port.to_string();
                    if ui
                        .add(egui::TextEdit::singleline(&mut port_str).desired_width(60.0))
                        .changed()
                        && let Ok(p) = port_str.parse::<u16>()
                    {
                        draft.bind_port = p;
                    }
                });

                if draft.kind != TunnelKind::Dynamic {
                    ui.horizontal(|ui| {
                        ui.label("Forward To Host:");
                        ui.text_edit_singleline(&mut draft.target_host);
                        ui.label("Port:");
                        let mut target_port_str = draft.target_port.to_string();
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut target_port_str)
                                    .desired_width(60.0),
                            )
                            .changed()
                            && let Ok(p) = target_port_str.parse::<u16>()
                        {
                            draft.target_port = p;
                        }
                    });
                }

                ui.checkbox(
                    &mut draft.auto_start,
                    "Auto-start tunnel when connecting to host",
                );

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(
                            RichText::new("Save Tunnel")
                                .strong()
                                .color(Color32::from_rgb(74, 222, 128)),
                        )
                        .clicked()
                    {
                        if let Err(e) = draft.validate() {
                            self.error_message = Some(e.to_string());
                        } else {
                            action = Some(TunnelAction::Save(draft.clone()));
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

pub fn is_port_in_use(addr: &str, port: u16) -> bool {
    std::net::TcpListener::bind((addr, port)).is_err()
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
