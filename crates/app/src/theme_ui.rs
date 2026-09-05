use eframe::egui::{self, Align, Color32, Layout, RichText, ScrollArea, Stroke};
use kervesh_core::{Settings, Store, TerminalPalette, TerminalProfile};

pub enum ThemeUiAction {
    ProfileUpdated,
    ExportPalette(String),
}

pub struct ThemeUi {
    pub open: bool,
    pub selected_preset: usize,
    pub editing_palette: TerminalPalette,
    pub active_profile_id: String,
    pub import_json_input: String,
    pub show_import: bool,
}

impl Default for ThemeUi {
    fn default() -> Self {
        Self::new()
    }
}

impl ThemeUi {
    pub fn new() -> Self {
        Self {
            open: false,
            selected_preset: 0,
            editing_palette: TerminalPalette::default(),
            active_profile_id: "default".into(),
            import_json_input: String::new(),
            show_import: false,
        }
    }

    pub fn open_for_profile(&mut self, profile: &TerminalProfile) {
        self.active_profile_id = profile.id.clone();
        self.editing_palette = profile.palette.clone();
        self.open = true;
    }

    pub fn render(
        &mut self,
        ctx: &egui::Context,
        settings: &mut Settings,
        store: &Store,
        action: &mut Option<ThemeUiAction>,
    ) {
        if !self.open {
            return;
        }

        let mut is_open = self.open;
        egui::Window::new("🎨 Theme Engine & ANSI Palette Editor")
            .open(&mut is_open)
            .collapsible(false)
            .resizable(true)
            .default_width(780.0)
            .default_height(560.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Preset Themes:").strong());
                    egui::ComboBox::from_id_salt("theme_preset_combo")
                        .selected_text(match self.selected_preset {
                            0 => "Kervesh Dark (Default)",
                            1 => "Kervesh Light",
                            2 => "Dracula",
                            3 => "Nord",
                            4 => "Tokyo Night",
                            5 => "Catppuccin Mocha",
                            6 => "Gruvbox Dark",
                            7 => "One Dark",
                            _ => "Custom",
                        })
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_value(
                                    &mut self.selected_preset,
                                    0,
                                    "Kervesh Dark (Default)",
                                )
                                .clicked()
                            {
                                self.editing_palette = TerminalPalette::default();
                            }
                            if ui
                                .selectable_value(&mut self.selected_preset, 1, "Kervesh Light")
                                .clicked()
                            {
                                self.editing_palette = TerminalPalette::light();
                            }
                            if ui
                                .selectable_value(&mut self.selected_preset, 2, "Dracula")
                                .clicked()
                            {
                                self.editing_palette = TerminalPalette::dracula();
                            }
                            if ui
                                .selectable_value(&mut self.selected_preset, 3, "Nord")
                                .clicked()
                            {
                                self.editing_palette = TerminalPalette::nord();
                            }
                            if ui
                                .selectable_value(&mut self.selected_preset, 4, "Tokyo Night")
                                .clicked()
                            {
                                self.editing_palette = TerminalPalette::tokyo_night();
                            }
                            if ui
                                .selectable_value(&mut self.selected_preset, 5, "Catppuccin Mocha")
                                .clicked()
                            {
                                self.editing_palette = TerminalPalette::catppuccin_mocha();
                            }
                            if ui
                                .selectable_value(&mut self.selected_preset, 6, "Gruvbox Dark")
                                .clicked()
                            {
                                self.editing_palette = TerminalPalette::gruvbox_dark();
                            }
                            if ui
                                .selectable_value(&mut self.selected_preset, 7, "One Dark")
                                .clicked()
                            {
                                self.editing_palette = TerminalPalette::one_dark();
                            }
                        });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("📋 Export JSON").clicked()
                            && let Ok(json) = self.editing_palette.export_json()
                        {
                            *action = Some(ThemeUiAction::ExportPalette(json));
                        }
                        if ui.button("📥 Import JSON").clicked() {
                            self.show_import = !self.show_import;
                        }
                    });
                });

                if self.show_import {
                    ui.add_space(4.0);
                    ui.group(|ui| {
                        ui.label(RichText::new("Paste Palette JSON:").strong());
                        ui.add(
                            egui::TextEdit::multiline(&mut self.import_json_input)
                                .desired_rows(3)
                                .desired_width(ui.available_width()),
                        );
                        ui.horizontal(|ui| {
                            if ui.button("Apply Imported Palette").clicked()
                                && let Ok(p) = TerminalPalette::import_json(&self.import_json_input)
                            {
                                self.editing_palette = p;
                                self.show_import = false;
                            }
                            if ui.button("Close").clicked() {
                                self.show_import = false;
                            }
                        });
                    });
                }

                ui.separator();

                ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                    ui.columns(2, |columns| {
                        // Left column: Color Pickers
                        columns[0].vertical(|ui| {
                            ui.heading("Primary Surfaces");
                            ui.add_space(4.0);

                            egui::Grid::new("primary_surfaces_grid")
                                .num_columns(2)
                                .spacing([10.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("Background:");
                                    Self::color_picker(ui, &mut self.editing_palette.background);
                                    ui.end_row();

                                    ui.label("Foreground:");
                                    Self::color_picker(ui, &mut self.editing_palette.foreground);
                                    ui.end_row();

                                    ui.label("Cursor:");
                                    Self::color_picker(ui, &mut self.editing_palette.cursor);
                                    ui.end_row();

                                    ui.label("Selection:");
                                    Self::color_picker(ui, &mut self.editing_palette.selection);
                                    ui.end_row();
                                });

                            ui.add_space(8.0);
                            ui.heading("16-Color ANSI Palette");
                            ui.add_space(4.0);

                            let names = [
                                "Black",
                                "Red",
                                "Green",
                                "Yellow",
                                "Blue",
                                "Magenta",
                                "Cyan",
                                "White",
                                "Bright Black",
                                "Bright Red",
                                "Bright Green",
                                "Bright Yellow",
                                "Bright Blue",
                                "Bright Magenta",
                                "Bright Cyan",
                                "Bright White",
                            ];

                            egui::Grid::new("ansi_palette_grid")
                                .num_columns(4)
                                .spacing([8.0, 4.0])
                                .show(ui, |ui| {
                                    for i in 0..8 {
                                        ui.label(RichText::new(names[i]).size(11.0));
                                        Self::color_picker(ui, &mut self.editing_palette.ansi[i]);

                                        ui.label(RichText::new(names[i + 8]).size(11.0));
                                        Self::color_picker(
                                            ui,
                                            &mut self.editing_palette.ansi[i + 8],
                                        );
                                        ui.end_row();
                                    }
                                });
                        });

                        // Right column: Contrast Checker & Live Terminal Preview
                        columns[1].vertical(|ui| {
                            ui.heading("Contrast & Accessibility");
                            ui.add_space(4.0);

                            let ratio = TerminalPalette::contrast_ratio(
                                self.editing_palette.foreground,
                                self.editing_palette.background,
                            );
                            let is_aa = ratio >= 4.5;
                            let is_aaa = ratio >= 7.0;

                            let (status_text, status_color) = if is_aaa {
                                (
                                    format!("{:.2}:1 (WCAG AAA Pass)", ratio),
                                    Color32::from_rgb(60, 210, 120),
                                )
                            } else if is_aa {
                                (
                                    format!("{:.2}:1 (WCAG AA Pass)", ratio),
                                    Color32::from_rgb(90, 160, 245),
                                )
                            } else {
                                (
                                    format!("{:.2}:1 (Low Contrast Warning)", ratio),
                                    Color32::from_rgb(235, 87, 87),
                                )
                            };

                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Contrast Ratio:").strong());
                                ui.label(RichText::new(status_text).color(status_color).strong());
                            });

                            ui.add_space(10.0);
                            ui.heading("Live Terminal Preview");
                            ui.add_space(4.0);

                            let bg = Color32::from_rgb(
                                self.editing_palette.background[0],
                                self.editing_palette.background[1],
                                self.editing_palette.background[2],
                            );
                            let fg = Color32::from_rgb(
                                self.editing_palette.foreground[0],
                                self.editing_palette.foreground[1],
                                self.editing_palette.foreground[2],
                            );

                            egui::Frame::group(ui.style())
                                .fill(bg)
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(80, 80, 80)))
                                .inner_margin(10.0)
                                .show(ui, |ui| {
                                    let green = Color32::from_rgb(
                                        self.editing_palette.ansi[2][0],
                                        self.editing_palette.ansi[2][1],
                                        self.editing_palette.ansi[2][2],
                                    );
                                    let blue = Color32::from_rgb(
                                        self.editing_palette.ansi[4][0],
                                        self.editing_palette.ansi[4][1],
                                        self.editing_palette.ansi[4][2],
                                    );
                                    let yellow = Color32::from_rgb(
                                        self.editing_palette.ansi[3][0],
                                        self.editing_palette.ansi[3][1],
                                        self.editing_palette.ansi[3][2],
                                    );
                                    let red = Color32::from_rgb(
                                        self.editing_palette.ansi[1][0],
                                        self.editing_palette.ansi[1][1],
                                        self.editing_palette.ansi[1][2],
                                    );
                                    let cyan = Color32::from_rgb(
                                        self.editing_palette.ansi[6][0],
                                        self.editing_palette.ansi[6][1],
                                        self.editing_palette.ansi[6][2],
                                    );

                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("user@kervesh-node")
                                                .color(green)
                                                .monospace(),
                                        );
                                        ui.label(RichText::new(":").color(fg).monospace());
                                        ui.label(
                                            RichText::new("~/production").color(blue).monospace(),
                                        );
                                        ui.label(RichText::new("$").color(fg).monospace());
                                        ui.label(
                                            RichText::new(" cargo test --workspace")
                                                .color(fg)
                                                .monospace()
                                                .strong(),
                                        );
                                    });

                                    ui.label(
                                        RichText::new("   Compiling kervesh-core v0.1.0...")
                                            .color(fg)
                                            .monospace(),
                                    );
                                    ui.label(
                                        RichText::new("   Compiling kervesh-ssh v0.1.0...")
                                            .color(fg)
                                            .monospace(),
                                    );
                                    ui.label(
                                        RichText::new("    Finished test [optimized] target(s)")
                                            .color(cyan)
                                            .monospace(),
                                    );
                                    ui.label(
                                        RichText::new("     Running 91 tests across 20 suites")
                                            .color(yellow)
                                            .monospace(),
                                    );
                                    ui.label(
                                        RichText::new(
                                            "test result: ok. 91 passed; 0 failed; 0 ignored",
                                        )
                                        .color(green)
                                        .monospace()
                                        .strong(),
                                    );
                                    ui.label(
                                        RichText::new("bash: exit status 0")
                                            .color(blue)
                                            .monospace()
                                            .size(10.5),
                                    );
                                    ui.label(
                                        RichText::new("WARN: memory pressure at 74%")
                                            .color(yellow)
                                            .monospace()
                                            .size(10.5),
                                    );
                                    ui.label(
                                        RichText::new("ERROR: disk space low on /var/log")
                                            .color(red)
                                            .monospace()
                                            .size(10.5),
                                    );
                                });
                        });
                    });
                });

                ui.add_space(10.0);
                ui.separator();

                ui.horizontal(|ui| {
                    if ui
                        .button(RichText::new(" Save Palette to Profile ").strong())
                        .clicked()
                        && let Some(p) = settings
                            .terminal_profiles
                            .iter_mut()
                            .find(|p| p.id == self.active_profile_id)
                    {
                        p.palette = self.editing_palette.clone();
                        let _ = store.save_settings(settings);
                        *action = Some(ThemeUiAction::ProfileUpdated);
                        self.open = false;
                    }
                    if ui.button("Close").clicked() {
                        self.open = false;
                    }
                });
            });

        self.open = is_open;
    }

    fn color_picker(ui: &mut egui::Ui, color: &mut [u8; 3]) {
        let mut egui_color = Color32::from_rgb(color[0], color[1], color[2]);
        if egui::color_picker::color_edit_button_srgba(
            ui,
            &mut egui_color,
            egui::color_picker::Alpha::Opaque,
        )
        .changed()
        {
            color[0] = egui_color.r();
            color[1] = egui_color.g();
            color[2] = egui_color.b();
        }
    }
}
