use crate::app::App;
use kervesh_core::{
    ClipboardProfile, MultilinePastePolicy, PaletteKind, TerminalCursor, TerminalPalette,
};

impl App {
    pub(crate) fn terminal_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Terminal");
        egui::ComboBox::from_id_salt("default-terminal-profile")
            .selected_text(&self.settings.terminal_profile(None).name)
            .show_ui(ui, |ui| {
                for profile in &self.settings.terminal_profiles {
                    ui.selectable_value(
                        &mut self.settings.default_terminal_profile,
                        profile.id.clone(),
                        &profile.name,
                    );
                }
            });
        ui.label(egui::RichText::new("Global default. Edit the selected profile below; Save applies it to sessions using that profile.").small().weak());
        let Some(profile) = self
            .settings
            .terminal_profiles
            .iter_mut()
            .find(|p| p.id == self.settings.default_terminal_profile)
        else {
            return;
        };
        egui::Grid::new("terminal-settings")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut profile.name);
                ui.end_row();
                ui.label("Font");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut profile.font_family).desired_width(200.0),
                    );
                    if ui.small_button("Browse…").clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("Font", &["ttf", "otf"])
                            .pick_file()
                    {
                        profile.font_family = path.to_string_lossy().into_owned();
                    }
                    if ui.small_button("Hack").clicked() {
                        profile.font_family = "Hack".into();
                    }
                });
                ui.end_row();
                ui.label("Size");
                ui.add(egui::Slider::new(&mut profile.font_size, 8.0..=32.0));
                ui.end_row();
                ui.label("Line height");
                ui.add(egui::Slider::new(&mut profile.line_height, 1.0..=2.0));
                ui.end_row();
                ui.label("Padding");
                ui.add(egui::Slider::new(&mut profile.padding, 0.0..=32.0));
                ui.end_row();
                ui.label("Scrollback");
                ui.add(egui::Slider::new(&mut profile.scrollback, 0..=100000).logarithmic(true));
                ui.end_row();
                ui.label("Cursor");
                ui.horizontal(|ui| {
                    for (value, label) in [
                        (TerminalCursor::Block, "Block"),
                        (TerminalCursor::Beam, "Beam"),
                        (TerminalCursor::Underline, "Underline"),
                    ] {
                        ui.selectable_value(&mut profile.cursor_style, value, label);
                    }
                    ui.checkbox(&mut profile.cursor_blink, "Blink");
                });
                ui.end_row();
                ui.label("Palette");
                ui.horizontal(|ui| {
                    let old = profile.palette.kind;
                    for (value, label) in [
                        (PaletteKind::KerveshDark, "Kervesh Dark"),
                        (PaletteKind::KerveshLight, "Kervesh Light"),
                        (PaletteKind::Custom, "Custom"),
                    ] {
                        ui.selectable_value(&mut profile.palette.kind, value, label);
                    }
                    if old != profile.palette.kind {
                        match profile.palette.kind {
                            PaletteKind::KerveshDark => {
                                profile.palette = TerminalPalette::default()
                            }
                            PaletteKind::KerveshLight => profile.palette = TerminalPalette::light(),
                            PaletteKind::Custom => {}
                        }
                    }
                });
                ui.end_row();
            });
        if profile.palette.kind == PaletteKind::Custom {
            ui.collapsing("Custom colors", |ui| {
                for (label, color) in [
                    ("Background", &mut profile.palette.background),
                    ("Foreground", &mut profile.palette.foreground),
                    ("Cursor", &mut profile.palette.cursor),
                    ("Selection", &mut profile.palette.selection),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(label);
                        ui.color_edit_button_srgb(color);
                    });
                }
                ui.horizontal_wrapped(|ui| {
                    for (index, color) in profile.palette.ansi.iter_mut().enumerate() {
                        ui.label(index.to_string());
                        ui.color_edit_button_srgb(color);
                    }
                });
            });
        }
        ui.collapsing("Font fallbacks", |ui| {
            ui.label(
                "Ordered local TTF/OTF files. Bundled Hack remains the final monospace fallback.",
            );
            let mut remove = None;
            for (index, path) in profile.font_fallbacks.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(path);
                    if ui.small_button("Remove").clicked() {
                        remove = Some(index);
                    }
                });
            }
            if let Some(index) = remove {
                profile.font_fallbacks.remove(index);
            }
            if profile.font_fallbacks.len() < 8
                && ui.small_button("Add fallback…").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("Font", &["ttf", "otf"])
                    .pick_file()
            {
                profile
                    .font_fallbacks
                    .push(path.to_string_lossy().into_owned());
            }
        });
        ui.separator();
        ui.strong("Clipboard");
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut profile.clipboard_profile,
                ClipboardProfile::Desktop,
                "Desktop",
            );
            ui.selectable_value(
                &mut profile.clipboard_profile,
                ClipboardProfile::Traditional,
                "Traditional",
            );
        });
        ui.checkbox(&mut profile.copy_on_select, "Copy on select");
        ui.checkbox(
            &mut profile.literal_control_keys,
            "Ctrl+Alt+letter sends literal control byte",
        );
        egui::ComboBox::from_id_salt("paste-policy")
            .selected_text(match profile.multiline_paste_policy {
                MultilinePastePolicy::Off => "Multiline protection off",
                MultilinePastePolicy::Warn => "Warn for multiline paste",
                MultilinePastePolicy::AlwaysPreview => "Always preview multiline paste",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut profile.multiline_paste_policy,
                    MultilinePastePolicy::Off,
                    "Off",
                );
                ui.selectable_value(
                    &mut profile.multiline_paste_policy,
                    MultilinePastePolicy::Warn,
                    "Warn for multiline paste",
                );
                ui.selectable_value(
                    &mut profile.multiline_paste_policy,
                    MultilinePastePolicy::AlwaysPreview,
                    "Always preview multiline paste",
                );
            });
        ui.separator();
        ui.strong("Behavior");
        ui.checkbox(
            &mut profile.hyperlinks_enabled,
            "Hyperlinks (Ctrl+Click or Open link)",
        );
        ui.checkbox(&mut profile.bell_visual, "Visual bell");
        ui.checkbox(&mut profile.bell_audio,"Audio bell").on_hover_text("Windows system sound; Linux requires paplay (PulseAudio or PipeWire compatibility).");
        ui.checkbox(&mut profile.follow_terminal_directory,"Follow terminal directory").on_hover_text("Requires OSC 7 directory metadata matching the connected host. Manual SFTP navigation pauses following.");
        for diagnostic in self.terminal_fonts.diagnostics() {
            ui.colored_label(
                crate::theme::colors::WARNING,
                format!(
                    "Font {}: {}. Using fallback.",
                    diagnostic.source, diagnostic.message
                ),
            );
        }
    }
}
