use crate::theme::colors;
use egui::RichText;
use kervesh_core::{Snippet, Store};
use std::collections::HashMap;

pub enum SnippetAction {
    InsertIntoActive(String),
    BroadcastToAll(String),
}

#[derive(Default)]
pub struct SnippetsUiState {
    pub manager_open: bool,
    pub search: String,
    pub selected_tag: Option<String>,
    pub editing_snippet: Option<Snippet>,
    pub run_dialog: Option<SnippetRunnerState>,
    pub broadcast_mode: bool,
    pub broadcast_input: String,
}

pub struct SnippetRunnerState {
    pub snippet: Snippet,
    pub placeholder_values: HashMap<String, String>,
}

impl SnippetRunnerState {
    pub fn new(snippet: Snippet) -> Self {
        let vars = snippet.extract_placeholders();
        let mut placeholder_values = HashMap::new();
        for v in vars {
            placeholder_values.insert(v, String::new());
        }
        Self {
            snippet,
            placeholder_values,
        }
    }

    pub fn rendered_command(&self) -> String {
        self.snippet.render(&self.placeholder_values)
    }
}

impl SnippetsUiState {
    pub fn show_manager(
        &mut self,
        ctx: &egui::Context,
        store: &Store,
        _dark: bool,
    ) -> Option<SnippetAction> {
        if !self.manager_open {
            return None;
        }

        let mut open = self.manager_open;
        let mut action = None;
        let mut delete_id = None;

        let snippets = store.snippets().unwrap_or_default();

        egui::Window::new("Command Snippets Library")
            .open(&mut open)
            .default_width(720.0)
            .default_height(480.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Search:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search)
                            .hint_text("Search snippets by name, command, tag…")
                            .desired_width(220.0),
                    );
                    if ui.button("Clear").clicked() {
                        self.search.clear();
                    }

                    ui.separator();
                    if ui.button("➕ New Snippet").clicked() {
                        self.editing_snippet = Some(Snippet::new("New Snippet", ""));
                    }
                });

                ui.separator();

                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let filtered: Vec<&Snippet> = snippets
                            .iter()
                            .filter(|s| {
                                if self.search.is_empty() {
                                    true
                                } else {
                                    s.matches(&self.search)
                                }
                            })
                            .collect();

                        if filtered.is_empty() {
                            ui.label(
                                RichText::new("No snippets found. Click 'New Snippet' to add one.")
                                    .weak(),
                            );
                        }

                        for snippet in filtered {
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(&snippet.name).strong());
                                    if !snippet.tags.is_empty() {
                                        ui.colored_label(
                                            colors::MUTED,
                                            format!("[{}]", snippet.tags),
                                        );
                                    }

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.button("🗑 Delete").clicked() {
                                                delete_id = Some(snippet.id.clone());
                                            }
                                            if ui.button("✏ Edit").clicked() {
                                                self.editing_snippet = Some(snippet.clone());
                                            }
                                            if ui.button("▶ Run").clicked() {
                                                if snippet.extract_placeholders().is_empty() {
                                                    action = Some(SnippetAction::InsertIntoActive(
                                                        snippet.command.clone(),
                                                    ));
                                                } else {
                                                    self.run_dialog = Some(
                                                        SnippetRunnerState::new(snippet.clone()),
                                                    );
                                                }
                                            }
                                            if ui.button("📡 Broadcast").clicked() {
                                                if snippet.extract_placeholders().is_empty() {
                                                    action = Some(SnippetAction::BroadcastToAll(
                                                        snippet.command.clone(),
                                                    ));
                                                } else {
                                                    self.run_dialog = Some(
                                                        SnippetRunnerState::new(snippet.clone()),
                                                    );
                                                }
                                            }
                                        },
                                    );
                                });

                                if !snippet.description.is_empty() {
                                    ui.label(RichText::new(&snippet.description).small().weak());
                                }

                                ui.monospace(&snippet.command);
                            });
                            ui.add_space(4.0);
                        }
                    });
            });

        if let Some(id) = delete_id {
            let _ = store.delete_snippet(&id);
        }

        // Snippet Edit Modal
        if let Some(mut snippet) = self.editing_snippet.take() {
            let mut edit_open = true;
            let mut save = false;
            let mut cancel = false;

            egui::Window::new("Edit Snippet")
                .open(&mut edit_open)
                .collapsible(false)
                .resizable(true)
                .default_width(500.0)
                .show(ctx, |ui| {
                    egui::Grid::new("snippet_form")
                        .num_columns(2)
                        .spacing([12.0, 8.0])
                        .show(ui, |ui| {
                            ui.label("Name");
                            ui.add(
                                egui::TextEdit::singleline(&mut snippet.name).desired_width(320.0),
                            );
                            ui.end_row();

                            ui.label("Tags");
                            ui.add(
                                egui::TextEdit::singleline(&mut snippet.tags)
                                    .hint_text("e.g. docker, logs, maintenance")
                                    .desired_width(320.0),
                            );
                            ui.end_row();

                            ui.label("Description");
                            ui.add(
                                egui::TextEdit::singleline(&mut snippet.description)
                                    .desired_width(320.0),
                            );
                            ui.end_row();

                            ui.label("Command");
                            ui.add(
                                egui::TextEdit::multiline(&mut snippet.command)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_rows(5)
                                    .desired_width(320.0)
                                    .hint_text("Use {{variable}} for parameter placeholders"),
                            );
                            ui.end_row();
                        });

                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(
                            "Tip: Placeholders in {{braces}} will be prompted before execution.",
                        )
                        .weak()
                        .small(),
                    );

                    ui.horizontal(|ui| {
                        save = ui.button("Save Snippet").clicked();
                        if ui.button("Cancel").clicked() {
                            cancel = true;
                        }
                    });
                });

            if cancel {
                edit_open = false;
            }

            if save {
                if let Err(e) = store.save_snippet(&snippet) {
                    eprintln!("Save snippet error: {e}");
                }
            } else if edit_open {
                self.editing_snippet = Some(snippet);
            }
        }

        self.manager_open = open;
        action
    }

    pub fn show_runner_modal(&mut self, ctx: &egui::Context) -> Option<SnippetAction> {
        let mut runner = self.run_dialog.take()?;

        let mut open = true;
        let mut insert_action = None;
        let mut broadcast_action = None;

        egui::Window::new(format!("Run Snippet — {}", runner.snippet.name))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(450.0)
            .show(ctx, |ui| {
                ui.label(RichText::new(&runner.snippet.description).weak());
                ui.separator();

                let keys: Vec<String> = runner.placeholder_values.keys().cloned().collect();
                egui::Grid::new("snippet_placeholders")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        for k in &keys {
                            ui.label(RichText::new(format!("{{{{{}}}}}", k)).monospace());
                            if let Some(v) = runner.placeholder_values.get_mut(k) {
                                ui.add(egui::TextEdit::singleline(v).desired_width(260.0));
                            }
                            ui.end_row();
                        }
                    });

                ui.separator();
                ui.label("Rendered Command Preview:");
                let rendered = runner.rendered_command();
                ui.monospace(&rendered);

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("▶ Insert into Active Terminal").clicked() {
                        insert_action = Some(rendered.clone());
                    }
                    if ui.button("📡 Broadcast to All Sessions").clicked() {
                        broadcast_action = Some(rendered.clone());
                    }
                });
            });

        if let Some(cmd) = insert_action {
            Some(SnippetAction::InsertIntoActive(cmd))
        } else if let Some(cmd) = broadcast_action {
            Some(SnippetAction::BroadcastToAll(cmd))
        } else if open {
            self.run_dialog = Some(runner);
            None
        } else {
            None
        }
    }
}
