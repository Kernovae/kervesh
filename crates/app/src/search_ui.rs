use egui::{Align, Color32, Layout, RichText, ScrollArea, Stroke, TextEdit, Vec2};
use kervesh_core::file_search::{SearchQuery, SearchResult};

#[derive(Debug, Clone)]
pub struct SearchUiState {
    pub open: bool,
    pub directory: String,
    pub pattern: String,
    pub extension: String,
    pub case_sensitive: bool,
    pub is_regex: bool,
    pub max_results: usize,
    pub is_searching: bool,
    pub results: Vec<SearchResult>,
    pub error: Option<String>,
}

impl Default for SearchUiState {
    fn default() -> Self {
        Self {
            open: false,
            directory: "/".into(),
            pattern: String::new(),
            extension: String::new(),
            case_sensitive: false,
            is_regex: false,
            max_results: 200,
            is_searching: false,
            results: Vec::new(),
            error: None,
        }
    }
}

pub enum SearchUiAction {
    ExecuteSearch(SearchQuery),
    OpenFile { path: String, line: usize },
    Close,
}

impl SearchUiState {
    pub fn open_for_path(&mut self, directory: impl Into<String>) {
        let d = directory.into();
        self.directory = if d.is_empty() { "/".into() } else { d };
        self.open = true;
        self.error = None;
    }

    pub fn set_results(&mut self, results: Vec<SearchResult>) {
        self.is_searching = false;
        self.results = results;
        self.error = None;
    }

    pub fn set_error(&mut self, err: String) {
        self.is_searching = false;
        self.error = Some(err);
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<SearchUiAction> {
        if !self.open {
            return None;
        }

        let mut action = None;
        let mut open_flag = self.open;

        egui::Window::new("Remote File Search (Grep)")
            .open(&mut open_flag)
            .default_size(Vec2::new(750.0, 520.0))
            .min_size(Vec2::new(500.0, 350.0))
            .resizable(true)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Directory:").strong());
                    ui.add(
                        TextEdit::singleline(&mut self.directory)
                            .desired_width(ui.available_width() - 80.0),
                    );
                });

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Search for:").strong());
                    let response = ui.add(
                        TextEdit::singleline(&mut self.pattern)
                            .hint_text("text or regular expression...")
                            .desired_width(ui.available_width() - 100.0),
                    );

                    let search_clicked = ui.button(RichText::new(" Search ").strong()).clicked();
                    let enter_pressed =
                        response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                    if (search_clicked || enter_pressed)
                        && !self.pattern.trim().is_empty()
                        && !self.is_searching
                    {
                        self.is_searching = true;
                        self.results.clear();
                        self.error = None;
                        let query = SearchQuery {
                            directory: self.directory.clone(),
                            pattern: self.pattern.clone(),
                            extension: if self.extension.trim().is_empty() {
                                None
                            } else {
                                Some(self.extension.trim().to_string())
                            },
                            case_sensitive: self.case_sensitive,
                            is_regex: self.is_regex,
                            max_results: self.max_results,
                        };
                        action = Some(SearchUiAction::ExecuteSearch(query));
                    }
                });

                ui.add_space(4.0);

                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut self.case_sensitive, "Match Case");
                    ui.add_space(8.0);
                    ui.checkbox(&mut self.is_regex, "Regex");
                    ui.add_space(16.0);
                    ui.label("Extension filter:");
                    ui.add(
                        TextEdit::singleline(&mut self.extension)
                            .hint_text("rs, py, toml")
                            .desired_width(100.0),
                    );
                    ui.add_space(8.0);
                    ui.label("Limit:");
                    ui.add(egui::DragValue::new(&mut self.max_results).range(10..=2000));
                });

                ui.separator();

                if let Some(err) = &self.error {
                    ui.colored_label(Color32::from_rgb(235, 87, 87), format!("Error: {err}"));
                    ui.add_space(4.0);
                }

                if self.is_searching {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Searching remote server...");
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("Results: {} matches found", self.results.len()))
                                .weak()
                                .size(11.0),
                        );
                    });
                }

                ui.add_space(4.0);

                // Results list
                ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                    if self.results.is_empty() && !self.is_searching {
                        ui.vertical_centered(|ui| {
                            ui.add_space(30.0);
                            ui.label(RichText::new("No search results yet").weak().italics());
                        });
                    }

                    for res in &self.results {
                        let frame = egui::Frame::group(ui.style())
                            .fill(ui.visuals().window_fill().gamma_multiply(0.4))
                            .stroke(Stroke::new(
                                1.0_f32,
                                ui.visuals().weak_text_color().gamma_multiply(0.2),
                            ))
                            .inner_margin(4.0);

                        frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let line_info = format!("{}:{}", res.path, res.line_number);

                                if ui
                                    .link(
                                        RichText::new(line_info)
                                            .color(Color32::from_rgb(90, 175, 255))
                                            .strong(),
                                    )
                                    .clicked()
                                {
                                    action = Some(SearchUiAction::OpenFile {
                                        path: res.path.clone(),
                                        line: res.line_number,
                                    });
                                }

                                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(res.line_content.trim())
                                            .monospace()
                                            .size(11.5),
                                    );
                                });
                            });
                        });
                        ui.add_space(2.0);
                    }
                });

                ui.add_space(8.0);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Close").clicked() {
                        action = Some(SearchUiAction::Close);
                    }
                });
            });

        if !open_flag {
            self.open = false;
            if action.is_none() {
                action = Some(SearchUiAction::Close);
            }
        }

        action
    }
}
