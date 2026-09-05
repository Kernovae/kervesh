use chrono::{DateTime, Utc};
use eframe::egui::{self, Align, Color32, Layout, RichText, ScrollArea, Stroke, TextEdit};
use kervesh_core::{AuditCommandEntry, Host, Store};

pub enum AuditUiAction {
    RunCommand(String),
    CopyCommand(String),
    ClearHistory,
}

pub struct AuditUi {
    pub open: bool,
    pub filter: String,
    pub selected_host_id: Option<String>,
    pub entries: Vec<AuditCommandEntry>,
}

impl Default for AuditUi {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditUi {
    pub fn new() -> Self {
        Self {
            open: false,
            filter: String::new(),
            selected_host_id: None,
            entries: Vec::new(),
        }
    }

    pub fn refresh(&mut self, store: &Store) {
        let results = if self.filter.is_empty() && self.selected_host_id.is_none() {
            store.audit_commands(200).unwrap_or_default()
        } else {
            store
                .search_audit_commands(&self.filter, self.selected_host_id.as_deref(), 200)
                .unwrap_or_default()
        };
        self.entries = results;
    }

    pub fn render(
        &mut self,
        ctx: &egui::Context,
        store: &Store,
        hosts: &[Host],
        action: &mut Option<AuditUiAction>,
    ) {
        if !self.open {
            return;
        }

        let mut is_open = self.open;
        egui::Window::new("📜 Unified Command History & Audit Trail")
            .open(&mut is_open)
            .collapsible(false)
            .resizable(true)
            .default_width(700.0)
            .default_height(480.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Search:").strong());
                    let filter_resp = ui.add(
                        TextEdit::singleline(&mut self.filter)
                            .hint_text("command or host…")
                            .desired_width(220.0),
                    );
                    if filter_resp.changed() {
                        self.refresh(store);
                    }

                    ui.add_space(8.0);
                    ui.label(RichText::new("Host:").strong());
                    egui::ComboBox::from_id_salt("audit_host_filter")
                        .selected_text(match &self.selected_host_id {
                            Some(hid) => hosts
                                .iter()
                                .find(|h| &h.id == hid)
                                .map(|h| h.name.as_str())
                                .unwrap_or("Specific Host"),
                            None => "All Hosts",
                        })
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_value(&mut self.selected_host_id, None, "All Hosts")
                                .clicked()
                            {
                                self.refresh(store);
                            }
                            for h in hosts {
                                if ui
                                    .selectable_value(
                                        &mut self.selected_host_id,
                                        Some(h.id.clone()),
                                        &h.name,
                                    )
                                    .clicked()
                                {
                                    self.refresh(store);
                                }
                            }
                        });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .button(
                                RichText::new("🗑 Clear Audit")
                                    .color(Color32::from_rgb(235, 87, 87)),
                            )
                            .clicked()
                        {
                            let _ = store.clear_audit_commands();
                            self.refresh(store);
                            *action = Some(AuditUiAction::ClearHistory);
                        }
                        if ui.button(" 🔄 Refresh ").clicked() {
                            self.refresh(store);
                        }
                    });
                });

                ui.separator();

                ui.label(
                    RichText::new(format!(
                        "Showing {} commands in audit log",
                        self.entries.len()
                    ))
                    .weak()
                    .size(11.0),
                );

                ui.add_space(4.0);

                ScrollArea::vertical()
                    .max_height(380.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.entries.is_empty() {
                            ui.vertical_centered(|ui| {
                                ui.add_space(30.0);
                                ui.label(
                                    RichText::new("No audit commands found matching criteria.")
                                        .weak()
                                        .italics(),
                                );
                            });
                            return;
                        }

                        for entry in &self.entries {
                            let frame = egui::Frame::group(ui.style())
                                .fill(ui.visuals().window_fill().gamma_multiply(0.4))
                                .stroke(Stroke::new(
                                    1.0_f32,
                                    ui.visuals().weak_text_color().gamma_multiply(0.15),
                                ))
                                .inner_margin(6.0);

                            frame.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let dt = DateTime::from_timestamp(entry.executed_at, 0)
                                        .map(|d: DateTime<Utc>| {
                                            d.format("%Y-%m-%d %H:%M:%S").to_string()
                                        })
                                        .unwrap_or_else(|| "Unknown".to_string());

                                    ui.label(RichText::new(dt).weak().size(11.0));

                                    ui.label(
                                        RichText::new(format!("[{}]", entry.host_label))
                                            .strong()
                                            .color(Color32::from_rgb(90, 160, 245))
                                            .size(11.5),
                                    );

                                    if let Some(code) = entry.exit_code {
                                        let (color, text) = if code == 0 {
                                            (Color32::from_rgb(60, 210, 120), "exit 0")
                                        } else {
                                            (Color32::from_rgb(235, 87, 87), "exit err")
                                        };
                                        ui.label(RichText::new(text).color(color).size(10.5));
                                    }

                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui.small_button("▶ Run").clicked() {
                                            *action = Some(AuditUiAction::RunCommand(
                                                entry.command.clone(),
                                            ));
                                        }
                                        if ui.small_button("📋 Copy").clicked() {
                                            *action = Some(AuditUiAction::CopyCommand(
                                                entry.command.clone(),
                                            ));
                                        }
                                    });
                                });

                                ui.add_space(2.0);
                                ui.label(
                                    RichText::new(&entry.command)
                                        .monospace()
                                        .strong()
                                        .size(12.0),
                                );
                            });
                            ui.add_space(3.0);
                        }
                    });
            });

        self.open = is_open;
    }
}
