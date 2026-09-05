use eframe::egui::{self, Align, Color32, Layout, RichText, ScrollArea, Stroke, TextEdit};
use kervesh_core::{Host, Store, TriggerAction, TriggerEngine, TriggerRule};

pub enum TriggerUiAction {
    TriggerSaved,
    TriggerDeleted,
}

pub struct TriggersUi {
    pub open: bool,
    pub editing_rule: Option<TriggerRule>,
    pub test_input: String,
    pub selected_action_kind: usize, // 0: Notification, 1: SendInput, 2: Highlight, 3: PlayBeep
    pub action_payload: String,
}

impl Default for TriggersUi {
    fn default() -> Self {
        Self::new()
    }
}

impl TriggersUi {
    pub fn new() -> Self {
        Self {
            open: false,
            editing_rule: None,
            test_input: String::new(),
            selected_action_kind: 0,
            action_payload: String::new(),
        }
    }

    pub fn render(
        &mut self,
        ctx: &egui::Context,
        store: &Store,
        hosts: &[Host],
        action: &mut Option<TriggerUiAction>,
    ) {
        if !self.open {
            return;
        }

        let mut is_open = self.open;
        let triggers = store.triggers().unwrap_or_default();

        egui::Window::new("⚡ Terminal Trigger-Action Rules")
            .open(&mut is_open)
            .collapsible(false)
            .resizable(true)
            .default_width(650.0)
            .default_height(480.0)
            .show(ctx, |ui| {
                if let Some(mut rule) = self.editing_rule.take() {
                    let keep = self.render_editor(ui, store, hosts, &mut rule, action);
                    if keep {
                        self.editing_rule = Some(rule);
                    }
                } else {
                    self.render_list(ui, store, hosts, &triggers, action);
                }
            });

        self.open = is_open;
    }

    fn render_list(
        &mut self,
        ui: &mut egui::Ui,
        store: &Store,
        hosts: &[Host],
        triggers: &[TriggerRule],
        action: &mut Option<TriggerUiAction>,
    ) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Automated Output Matchers & Event Actions").strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .button(RichText::new(" + New Trigger Rule ").strong())
                    .clicked()
                {
                    self.editing_rule = Some(TriggerRule::new(
                        "New Alert",
                        "ERROR",
                        false,
                        TriggerAction::Notification("Error detected on server".into()),
                    ));
                    self.selected_action_kind = 0;
                    self.action_payload = "Error detected on server".into();
                }
            });
        });

        ui.separator();

        if triggers.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    RichText::new("No trigger rules configured yet. Create one to automate actions on terminal output.")
                        .weak()
                        .italics(),
                );
            });
            return;
        }

        ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
            for t in triggers {
                let frame = egui::Frame::group(ui.style())
                    .fill(ui.visuals().window_fill().gamma_multiply(0.4))
                    .stroke(Stroke::new(
                        1.0_f32,
                        ui.visuals().weak_text_color().gamma_multiply(0.15),
                    ))
                    .inner_margin(6.0);

                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let mut enabled = t.enabled;
                        if ui.checkbox(&mut enabled, "").changed() {
                            let mut updated = t.clone();
                            updated.enabled = enabled;
                            let _ = store.save_trigger(&updated);
                            *action = Some(TriggerUiAction::TriggerSaved);
                        }

                        ui.label(RichText::new(&t.name).strong());

                        if t.is_regex {
                            ui.label(
                                RichText::new("REGEX")
                                    .color(Color32::from_rgb(180, 100, 240))
                                    .size(10.5),
                            );
                        }

                        if let Some(hid) = &t.host_id {
                            let hname = hosts
                                .iter()
                                .find(|h| &h.id == hid)
                                .map(|h| h.name.as_str())
                                .unwrap_or("Host");
                            ui.label(
                                RichText::new(format!("[{}]", hname))
                                    .color(Color32::from_rgb(90, 160, 245))
                                    .size(10.5),
                            );
                        }

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .button(RichText::new("🗑").color(Color32::from_rgb(235, 87, 87)))
                                .clicked()
                            {
                                let _ = store.delete_trigger(&t.id);
                                *action = Some(TriggerUiAction::TriggerDeleted);
                            }
                            if ui.button("✏ Edit").clicked() {
                                match &t.action {
                                    TriggerAction::Notification(p) => {
                                        self.selected_action_kind = 0;
                                        self.action_payload = p.clone();
                                    }
                                    TriggerAction::SendInput(p) => {
                                        self.selected_action_kind = 1;
                                        self.action_payload = p.clone();
                                    }
                                    TriggerAction::Highlight(p) => {
                                        self.selected_action_kind = 2;
                                        self.action_payload = p.clone();
                                    }
                                    TriggerAction::PlayBeep => {
                                        self.selected_action_kind = 3;
                                        self.action_payload.clear();
                                    }
                                }
                                self.editing_rule = Some(t.clone());
                            }
                        });
                    });

                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("Pattern: \"{}\"", t.pattern))
                                .monospace()
                                .weak(),
                        );
                        ui.label(RichText::new("→").weak());
                        ui.label(RichText::new(t.action.display_name()).strong());
                    });
                });
                ui.add_space(4.0);
            }
        });
    }

    fn render_editor(
        &mut self,
        ui: &mut egui::Ui,
        store: &Store,
        hosts: &[Host],
        rule: &mut TriggerRule,
        action: &mut Option<TriggerUiAction>,
    ) -> bool {
        let mut keep = true;
        ui.horizontal(|ui| {
            if ui.button("⬅ Back to Rules").clicked() {
                keep = false;
            }
            ui.label(RichText::new("Edit Trigger Rule").strong());
        });

        if !keep {
            return false;
        }

        ui.separator();

        egui::Grid::new("trigger_editor_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(RichText::new("Rule Name:").strong());
                ui.text_edit_singleline(&mut rule.name);
                ui.end_row();

                ui.label(RichText::new("Match Pattern:").strong());
                ui.text_edit_singleline(&mut rule.pattern);
                ui.end_row();

                ui.label("");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut rule.is_regex, "Regex Match");
                    ui.checkbox(&mut rule.case_sensitive, "Case Sensitive");
                });
                ui.end_row();

                ui.label(RichText::new("Target Host:").strong());
                egui::ComboBox::from_id_salt("trigger_host_select")
                    .selected_text(match &rule.host_id {
                        Some(hid) => hosts
                            .iter()
                            .find(|h| &h.id == hid)
                            .map(|h| h.name.as_str())
                            .unwrap_or("Specific Host"),
                        None => "All Hosts (Global)",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut rule.host_id, None, "All Hosts (Global)");
                        for h in hosts {
                            ui.selectable_value(&mut rule.host_id, Some(h.id.clone()), &h.name);
                        }
                    });
                ui.end_row();

                ui.label(RichText::new("Action:").strong());
                egui::ComboBox::from_id_salt("trigger_action_kind")
                    .selected_text(match self.selected_action_kind {
                        0 => "Desktop / In-App Notification",
                        1 => "Send Automated Input to Terminal",
                        2 => "Highlight Output",
                        3 => "Play Alert Sound",
                        _ => "Unknown",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.selected_action_kind,
                            0,
                            "Desktop / In-App Notification",
                        );
                        ui.selectable_value(
                            &mut self.selected_action_kind,
                            1,
                            "Send Automated Input to Terminal",
                        );
                        ui.selectable_value(&mut self.selected_action_kind, 2, "Highlight Output");
                        ui.selectable_value(&mut self.selected_action_kind, 3, "Play Alert Sound");
                    });
                ui.end_row();

                if self.selected_action_kind != 3 {
                    let label_text = match self.selected_action_kind {
                        0 => "Notification Msg:",
                        1 => "Input to Send:",
                        2 => "Highlight Color:",
                        _ => "Payload:",
                    };
                    ui.label(RichText::new(label_text).strong());
                    ui.text_edit_singleline(&mut self.action_payload);
                    ui.end_row();
                }
            });

        ui.add_space(8.0);
        ui.separator();
        ui.label(RichText::new("🧪 Test Rule Against Sample Output:").strong());
        ui.add(
            TextEdit::multiline(&mut self.test_input)
                .hint_text("Paste terminal output here to test rule match…")
                .desired_rows(3)
                .desired_width(ui.available_width()),
        );

        // Run live test
        let test_action = match self.selected_action_kind {
            0 => TriggerAction::Notification(self.action_payload.clone()),
            1 => TriggerAction::SendInput(self.action_payload.clone()),
            2 => TriggerAction::Highlight(self.action_payload.clone()),
            3 => TriggerAction::PlayBeep,
            _ => TriggerAction::PlayBeep,
        };
        let mut test_rule = rule.clone();
        test_rule.action = test_action;

        let test_engine = TriggerEngine::new(&[test_rule]);
        let triggered = test_engine.evaluate(&self.test_input, None);

        if !self.test_input.is_empty() {
            if !triggered.is_empty() {
                ui.label(
                    RichText::new(format!(
                        "✓ MATCH FOUND! Will execute: {}",
                        triggered[0].display_name()
                    ))
                    .color(Color32::from_rgb(60, 210, 120))
                    .strong(),
                );
            } else {
                ui.label(
                    RichText::new("✗ No match on sample output.")
                        .color(Color32::from_rgb(235, 87, 87)),
                );
            }
        }

        ui.add_space(12.0);
        let mut keep = true;
        ui.horizontal(|ui| {
            if ui
                .button(RichText::new(" Save Trigger Rule ").strong())
                .clicked()
            {
                let final_action = match self.selected_action_kind {
                    0 => TriggerAction::Notification(self.action_payload.clone()),
                    1 => TriggerAction::SendInput(self.action_payload.clone()),
                    2 => TriggerAction::Highlight(self.action_payload.clone()),
                    3 => TriggerAction::PlayBeep,
                    _ => TriggerAction::PlayBeep,
                };
                rule.action = final_action;
                let _ = store.save_trigger(rule);
                *action = Some(TriggerUiAction::TriggerSaved);
                keep = false;
            }
            if ui.button("Cancel").clicked() {
                keep = false;
            }
        });
        keep
    }
}
