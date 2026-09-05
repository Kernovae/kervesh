use crate::app::{App, Confirmation};
use egui::RichText;
use kervesh_core::{AuthMethod, secrets};
use kervesh_ssh::FileOperation;

impl App {
    pub(crate) fn host_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("hosts")
            .default_width(238.0)
            .width_range(190.0..=400.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("CONNECTIONS").small().strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("+").clicked() {
                            self.open_new_host();
                        }
                    });
                });
                ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text("Search hosts, groups, tags")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(8.0);
                let mut connect = None;
                let mut edit = None;
                let mut delete = None;
                let mut duplicate = None;
                let mut favorite = None;
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut groups: Vec<_> = self
                        .hosts
                        .iter()
                        .filter(|h| h.matches(&self.query))
                        .map(|h| h.group.clone())
                        .collect();
                    groups.sort();
                    groups.dedup();
                    for group in groups {
                        egui::CollapsingHeader::new(if group.is_empty() {
                            "Ungrouped"
                        } else {
                            &group
                        })
                        .default_open(true)
                        .show(ui, |ui| {
                            for host in self
                                .hosts
                                .iter()
                                .filter(|h| h.group == group && h.matches(&self.query))
                            {
                                let connected = self
                                    .tabs
                                    .iter()
                                    .any(|t| t.host.id == host.id && t.connected);
                                let response = ui.selectable_label(
                                    connected,
                                    format!(
                                        "{}  {}",
                                        if host.favorite {
                                            "★"
                                        } else if connected {
                                            "●"
                                        } else {
                                            "○"
                                        },
                                        host.name
                                    ),
                                );
                                if response.double_clicked() {
                                    connect = Some(host.clone());
                                }
                                response
                                    .on_hover_text(format!(
                                        "{}@{}:{}\n{}\nDouble-click to connect",
                                        host.username, host.hostname, host.port, host.tags
                                    ))
                                    .context_menu(|ui| {
                                        if ui.button("Connect").clicked() {
                                            connect = Some(host.clone());
                                            ui.close();
                                        }
                                        if ui.button("Edit").clicked() {
                                            edit = Some(host.clone());
                                            ui.close();
                                        }
                                        if ui.button("Duplicate").clicked() {
                                            duplicate = Some(host.duplicate());
                                            ui.close();
                                        }
                                        if ui
                                            .button(if host.favorite {
                                                "Remove favorite"
                                            } else {
                                                "Favorite"
                                            })
                                            .clicked()
                                        {
                                            let mut h = host.clone();
                                            h.favorite = !h.favorite;
                                            favorite = Some(h);
                                            ui.close();
                                        }
                                        if ui.button("Delete…").clicked() {
                                            delete = Some(host.clone());
                                            ui.close();
                                        }
                                    });
                                ui.label(
                                    RichText::new(format!("{}@{}", host.username, host.hostname))
                                        .small()
                                        .weak(),
                                );
                            }
                        });
                    }
                    if self.hosts.is_empty() {
                        ui.label(
                            RichText::new("Save your first connection to get started.").weak(),
                        );
                    }
                });
                ui.add_space(12.0);
                if ui.button("+  New connection").clicked() {
                    self.open_new_host();
                }
                ui.label(
                    RichText::new(format!(
                        "{} hosts · {} sessions",
                        self.hosts.len(),
                        self.tabs.len()
                    ))
                    .small()
                    .weak(),
                );
                if let Some(host) = connect {
                    self.begin_connect(host, ctx);
                }
                if let Some(host) = edit.or(duplicate) {
                    self.host_form = Some(host);
                    self.form_error = None;
                }
                if let Some(host) = favorite {
                    if let Err(e) = self.store.save_host(&host) {
                        self.notice = Some(e.to_string());
                    }
                    self.refresh_hosts();
                }
                if let Some(host) = delete {
                    self.confirmation = Some(Confirmation::DeleteHost(host));
                }
            });
    }
    pub(crate) fn host_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut host) = self.host_form.take() else {
            return;
        };
        let mut open = true;
        let mut save = false;
        let mut test = false;
        egui::Window::new("Connection profile")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .show(ctx, |ui| {
                egui::Grid::new("host_form")
                    .num_columns(2)
                    .spacing([16.0, 10.0])
                    .show(ui, |ui| {
                        field(ui, "Name", &mut host.name);
                        field(ui, "Hostname / IP", &mut host.hostname);
                        ui.label("Port");
                        ui.add(egui::DragValue::new(&mut host.port).range(1..=65535));
                        ui.end_row();
                        field(ui, "Username", &mut host.username);
                        ui.label("Authentication");
                        egui::ComboBox::from_id_salt("auth")
                            .selected_text(auth_label(&host.auth))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut host.auth,
                                    AuthMethod::Password,
                                    "Password",
                                );
                                ui.selectable_value(
                                    &mut host.auth,
                                    AuthMethod::PrivateKey,
                                    "Private key",
                                );
                                ui.selectable_value(&mut host.auth, AuthMethod::Agent, "SSH agent");
                            });
                        ui.end_row();
                        if host.auth == AuthMethod::PrivateKey {
                            ui.label("Private key");
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut host.key_path)
                                        .desired_width(230.0),
                                );
                                if ui.button("Browse…").clicked()
                                    && let Some(path) = rfd::FileDialog::new().pick_file()
                                {
                                    host.key_path = path.to_string_lossy().into_owned();
                                }
                            });
                            ui.end_row();
                        }
                        field(ui, "Group", &mut host.group);
                        field(ui, "Tags", &mut host.tags);
                        ui.label("Timeout (seconds)");
                        ui.add(egui::DragValue::new(&mut host.timeout_secs).range(1..=300));
                        ui.end_row();
                        ui.label("Keepalive (0 = off)");
                        ui.add(egui::DragValue::new(&mut host.keepalive_secs).range(0..=3600));
                        ui.end_row();
                        ui.label("Options");
                        ui.vertical(|ui| {
                            ui.checkbox(&mut host.favorite, "Favorite");
                            ui.checkbox(
                                &mut host.auto_reconnect,
                                "Reconnect after connection loss",
                            );
                        });
                        ui.end_row();
                    });
                ui.add_space(8.0);
                ui.label(
                    RichText::new(
                        "Credentials are requested when connecting. Profiles contain no passwords.",
                    )
                    .small()
                    .weak(),
                );
                if let Some(error) = &self.form_error {
                    ui.colored_label(crate::theme::colors::DANGER, error);
                }
                ui.horizontal(|ui| {
                    save = ui.button("Save connection").clicked();
                    test = ui.button("Test connection…").clicked();
                });
            });
        if save {
            match self.store.save_host(&host) {
                Ok(()) => {
                    open = false;
                    self.refresh_hosts();
                }
                Err(e) => self.form_error = Some(e.to_string()),
            }
        }
        if test {
            self.begin_connect(host.clone(), ctx);
        }
        if open {
            self.host_form = Some(host);
        }
    }
    pub(crate) fn login_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut login) = self.login.take() else {
            return;
        };
        let mut open = true;
        let mut connect = false;
        egui::Window::new("Connect securely")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(380.0)
            .show(ctx, |ui| {
                ui.heading(&login.host.name);
                ui.monospace(format!(
                    "{}@{}:{}",
                    login.host.username, login.host.hostname, login.host.port
                ));
                ui.separator();
                if login.host.auth != AuthMethod::Agent {
                    ui.label(if login.host.auth == AuthMethod::PrivateKey {
                        "Passphrase (leave empty for an unencrypted key)"
                    } else {
                        "Password"
                    });
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut *login.credentials.secret)
                            .password(true)
                            .desired_width(f32::INFINITY),
                    );
                    ui.checkbox(
                        &mut login.credentials.remember,
                        "Save in operating system credential store",
                    );
                    connect =
                        response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                } else {
                    ui.label("Uses keys from the local SSH agent. Agent forwarding is disabled.");
                }
                connect |= ui.button("Connect").clicked();
            });
        if connect {
            self.host_form = None;
            self.connect(login, ctx);
        } else if open {
            self.login = Some(login);
        }
    }
    pub(crate) fn trust_dialog(&mut self, ctx: &egui::Context) {
        let Some(prompt) = self.trust.front() else {
            return;
        };
        let mut decision = None;
        egui::Window::new("Verify server identity")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(RichText::new("First connection to this endpoint").strong());
                ui.monospace(format!("{}:{}", prompt.host, prompt.port));
                ui.label(
                    "Compare this SHA-256 fingerprint with a trusted source before continuing.",
                );
                ui.add_space(8.0);
                ui.monospace(&prompt.fingerprint);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Reject").clicked() {
                        decision = Some(false);
                    }
                    if ui.button("Trust key and connect").clicked() {
                        decision = Some(true);
                    }
                });
            });
        if let Some(accepted) = decision
            && let Some(prompt) = self.trust.pop_front()
        {
            let _ = prompt.reply.send(accepted);
        }
    }
    pub(crate) fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(confirmation) = self.confirmation.take() else {
            return;
        };
        let mut choice = None;
        let message = match &confirmation {
            Confirmation::DeleteHost(host) => {
                format!("Delete profile ‘{}’ and its saved credential?", host.name)
            }
            Confirmation::CloseTab(_) => {
                "Close this session? Active transfers will be cancelled.".into()
            }
            Confirmation::File(_, FileOperation::Delete(path, dir)) => format!(
                "Permanently delete {} ‘{path}’? Directories must be empty.",
                if *dir { "directory" } else { "file" }
            ),
            Confirmation::File(_, _) => "Apply this remote file operation?".into(),
            Confirmation::Transfer(_, request) => format!(
                "Replace destination for transfer?\nLocal: {}\nRemote: {}",
                request.local.display(),
                request.remote
            ),
            Confirmation::ForgetTrust(host, port) => format!(
                "Remove trusted fingerprint for {host}:{port}? Verify the replacement key before trusting it."
            ),
            Confirmation::Quit => {
                "Close all sessions and quit? Active transfers will be cancelled.".into()
            }
        };
        egui::Window::new("Confirm action")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(message);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        choice = Some(false);
                    }
                    if ui.button("Confirm").clicked() {
                        choice = Some(true);
                    }
                });
            });
        match choice {
            None => self.confirmation = Some(confirmation),
            Some(false) => {}
            Some(true) => match confirmation {
                Confirmation::DeleteHost(host) => {
                    if let Err(e) = secrets::delete(&host.id) {
                        self.notice = Some(format!(
                            "Credential cleanup unavailable; remove any saved entry from the OS keyring: {e}"
                        ));
                    }
                    if let Err(e) = self.store.delete_host(&host.id) {
                        self.notice = Some(e.to_string());
                    }
                    self.refresh_hosts();
                }
                Confirmation::CloseTab(id) => {
                    if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
                        for transfer in &self.tabs[index].transfers {
                            transfer.request.cancel.cancel();
                        }
                        self.tabs.remove(index);
                        self.active = self.active.min(self.tabs.len().saturating_sub(1));
                    }
                }
                Confirmation::File(id, operation) => {
                    self.send(id, kervesh_ssh::Command::File(operation))
                }
                Confirmation::Transfer(id, mut request) => {
                    request.overwrite = true;
                    self.queue_transfer(id, request);
                }
                Confirmation::ForgetTrust(host, port) => {
                    if let Err(e) = self.store.forget_trust(&host, port) {
                        self.notice = Some(e.to_string());
                    }
                }
                Confirmation::Quit => {
                    self.allow_quit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            },
        }
    }
}
fn field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.label(label);
    ui.add(egui::TextEdit::singleline(value).desired_width(280.0));
    ui.end_row();
}
fn auth_label(auth: &AuthMethod) -> &'static str {
    match auth {
        AuthMethod::Password => "Password",
        AuthMethod::PrivateKey => "Private key",
        AuthMethod::Agent => "SSH agent",
    }
}
