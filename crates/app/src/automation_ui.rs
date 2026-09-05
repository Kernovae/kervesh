use egui::{Align2, Color32, RichText, Vec2};
use kervesh_core::{AutomationMacro, Host, MacroStep};

#[derive(Default)]
pub struct AutomationUi {
    pub editing: Option<AutomationMacro>,
    pub filter: String,
    pub error_message: Option<String>,
}

pub enum AutomationAction {
    RunOnActive(AutomationMacro),
    Save(AutomationMacro),
    Delete(String),
}

impl AutomationUi {
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        macros: &[AutomationMacro],
        hosts: &[Host],
        is_open: &mut bool,
    ) -> Option<AutomationAction> {
        let mut action = None;
        if !*is_open {
            return None;
        }

        let mut modal_open = *is_open;
        egui::Window::new(RichText::new("Automation Macros & Key Sequences").strong())
            .open(&mut modal_open)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .resizable(true)
            .default_size(Vec2::new(720.0, 520.0))
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

                    if ui.button(RichText::new("➕ New Macro").color(Color32::from_rgb(74, 222, 128))).clicked() {
                        self.editing = Some(AutomationMacro::new("New Macro", ""));
                    }
                });

                ui.separator();

                let filtered: Vec<&AutomationMacro> = macros
                    .iter()
                    .filter(|m| {
                        if self.filter.is_empty() {
                            return true;
                        }
                        let query = self.filter.to_lowercase();
                        m.name.to_lowercase().contains(&query) || m.description.to_lowercase().contains(&query)
                    })
                    .collect();

                if filtered.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(32.0);
                        ui.label(RichText::new("No automation macros created").italics().color(Color32::GRAY));
                        ui.label("Create login sequences, post-connect scripts, delays, and prompt expectations");
                    });
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(380.0)
                        .show(ui, |ui| {
                            for mac in filtered {
                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(RichText::new(&mac.name).strong());
                                                ui.label(
                                                    RichText::new(format!("({} steps)", mac.steps.len()))
                                                        .size(11.0)
                                                        .color(Color32::LIGHT_GRAY),
                                                );
                                                if mac.run_on_connect {
                                                    ui.label(
                                                        RichText::new("⚡ Run on Connect")
                                                            .size(10.0)
                                                            .color(Color32::from_rgb(250, 204, 21)),
                                                    );
                                                }
                                            });

                                            if !mac.description.is_empty() {
                                                ui.label(RichText::new(&mac.description).size(12.0).color(Color32::GRAY));
                                            }

                                            // Steps preview
                                            let steps_summary: Vec<String> = mac
                                                .steps
                                                .iter()
                                                .take(3)
                                                .map(|s| match s {
                                                    MacroStep::SendText { text, .. } => format!("Send: \"{text}\""),
                                                    MacroStep::DelayMs(ms) => format!("Delay: {ms}ms"),
                                                    MacroStep::ExpectPrompt(p) => format!("Expect: \"{p}\""),
                                                })
                                                .collect();
                                            if !steps_summary.is_empty() {
                                                ui.label(
                                                    RichText::new(steps_summary.join(" ➔ "))
                                                        .size(11.0)
                                                        .color(Color32::from_rgb(147, 197, 253)),
                                                );
                                            }
                                        });

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.button(RichText::new("▶ Run Now").color(Color32::from_rgb(34, 197, 94))).clicked() {
                                                action = Some(AutomationAction::RunOnActive(mac.clone()));
                                            }

                                            if ui.button("✏ Edit").clicked() {
                                                self.editing = Some(mac.clone());
                                            }

                                            if ui.button(RichText::new("🗑").color(Color32::GRAY)).clicked() {
                                                action = Some(AutomationAction::Delete(mac.id.clone()));
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
                "New Automation Macro"
            } else {
                "Edit Macro"
            })
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .resizable(false)
            .default_size(Vec2::new(560.0, 480.0))
            .show(ctx, |ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut draft.name);

                ui.label("Description:");
                ui.text_edit_singleline(&mut draft.description);

                ui.label("Bind to Host (Optional):");
                egui::ComboBox::from_id_salt("macro_host_bind")
                    .selected_text(
                        draft
                            .host_id
                            .as_ref()
                            .and_then(|id| {
                                hosts.iter().find(|h| &h.id == id).map(|h| h.name.as_str())
                            })
                            .unwrap_or("All Hosts (Global)"),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut draft.host_id, None, "All Hosts (Global)");
                        for host in hosts {
                            ui.selectable_value(
                                &mut draft.host_id,
                                Some(host.id.clone()),
                                &host.name,
                            );
                        }
                    });

                ui.checkbox(
                    &mut draft.run_on_connect,
                    "Auto-run sequence when connection establishes",
                );

                ui.separator();
                ui.label(RichText::new("Sequence Steps:").strong());

                let mut remove_step_idx = None;
                egui::ScrollArea::vertical()
                    .max_height(180.0)
                    .show(ui, |ui| {
                        for (idx, step) in draft.steps.iter_mut().enumerate() {
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(format!("{}.", idx + 1));
                                    match step {
                                        MacroStep::SendText {
                                            text,
                                            append_newline,
                                        } => {
                                            ui.label("Send:");
                                            ui.text_edit_singleline(text);
                                            ui.checkbox(append_newline, "+ Enter");
                                        }
                                        MacroStep::DelayMs(ms) => {
                                            ui.label("Delay (ms):");
                                            let mut ms_str = ms.to_string();
                                            if ui
                                                .add(
                                                    egui::TextEdit::singleline(&mut ms_str)
                                                        .desired_width(60.0),
                                                )
                                                .changed()
                                                && let Ok(v) = ms_str.parse::<u64>()
                                            {
                                                *ms = v;
                                            }
                                        }
                                        MacroStep::ExpectPrompt(prompt) => {
                                            ui.label("Wait For Prompt:");
                                            ui.text_edit_singleline(prompt);
                                        }
                                    }

                                    if ui.button("✕").clicked() {
                                        remove_step_idx = Some(idx);
                                    }
                                });
                            });
                        }
                    });

                if let Some(i) = remove_step_idx
                    && i < draft.steps.len()
                {
                    draft.steps.remove(i);
                }

                ui.horizontal(|ui| {
                    if ui.button("➕ Add Send Text").clicked() {
                        draft.steps.push(MacroStep::SendText {
                            text: "echo hello".into(),
                            append_newline: true,
                        });
                    }
                    if ui.button("➕ Add Delay").clicked() {
                        draft.steps.push(MacroStep::DelayMs(500));
                    }
                    if ui.button("➕ Add Expect Prompt").clicked() {
                        draft.steps.push(MacroStep::ExpectPrompt("$".into()));
                    }
                });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(
                            RichText::new("Save Macro")
                                .strong()
                                .color(Color32::from_rgb(74, 222, 128)),
                        )
                        .clicked()
                    {
                        if let Err(e) = draft.validate() {
                            self.error_message = Some(e.to_string());
                        } else {
                            action = Some(AutomationAction::Save(draft.clone()));
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
