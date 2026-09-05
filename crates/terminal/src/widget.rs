use crate::{
    ClipboardIntent, MultilinePastePolicy, Terminal, TerminalAction, clipboard_intent, encode_key,
    encode_paste,
};
use alacritty_terminal::{
    grid::Scroll,
    index::{Column, Point, Side},
    selection::{Selection, SelectionType},
    term::{TermMode, viewport_to_point},
};
use egui::{Event, Key, Rect, Sense, Ui, vec2};

impl Terminal {
    pub fn ui(&mut self, ui: &mut Ui, _legacy_font_size: f32) -> TerminalAction {
        self.show(ui, false)
    }
    pub fn show(&mut self, ui: &mut Ui, sftp_available: bool) -> TerminalAction {
        let mut action = TerminalAction::default();
        if self.search.open && self.search.dirty {
            self.refresh_search();
        }
        if self.search.open {
            ui.horizontal(|ui| {
                let field = ui.add(
                    egui::TextEdit::singleline(&mut self.search.query)
                        .hint_text("Search scrollback")
                        .desired_width(180.0),
                );
                if self.search.focus {
                    field.request_focus();
                    self.search.focus = false;
                }
                let changed =
                    field.changed() | ui.checkbox(&mut self.search.case_sensitive, "Aa").changed();
                if changed {
                    self.refresh_search();
                    if let Some(m) = self.search.matches.first() {
                        self.term.scroll_to_point(m.start);
                    }
                }
                if ui.small_button("Previous").clicked() {
                    self.navigate_match(false);
                }
                if ui.small_button("Next").clicked()
                    || (field.has_focus() && ui.input(|i| i.key_pressed(Key::Enter)))
                {
                    self.navigate_match(true);
                }
                ui.label(format!(
                    "{}{} matches",
                    self.search.matches.len(),
                    if self.search.matches.len() == 10000 {
                        "+"
                    } else {
                        ""
                    }
                ));
                if ui.small_button("Close").clicked()
                    || (field.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)))
                {
                    self.search.open = false;
                    self.restore_focus = true;
                }
            });
        }
        let font = self.font_config.font_id(self.profile.font_size, false);
        let cell = crate::cell_metrics(ui.ctx(), &font, self.profile.line_height);
        let size = ui.available_size().max(vec2(cell.x * 2.0, cell.y));
        let (outer, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        ui.painter().rect_filled(
            outer,
            0.0,
            crate::renderer::rgb(self.profile.palette.background),
        );
        let rect = outer.shrink(
            self.profile
                .padding
                .min(outer.width().min(outer.height()) / 4.0),
        );
        let cols = ((rect.width() / cell.x).floor() as usize).max(2);
        let rows = ((rect.height() / cell.y).floor() as usize).max(1);
        if (cols, rows) != (self.cols, self.rows) {
            self.resize(cols, rows);
            action.resize = Some((cols as u32, rows as u32));
        }
        let blocked = self.pending_paste.is_some();
        if self.restore_focus && ui.is_enabled() {
            response.request_focus();
            self.restore_focus = false;
        }
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                response.id,
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            )
        });
        if !blocked && (response.clicked() || response.drag_started()) {
            response.request_focus();
        }
        let mode = *self.term.mode();
        let mouse_mode = mode.intersects(TermMode::MOUSE_MODE) && !ui.input(|i| i.modifiers.shift);
        let location = |pos: egui::Pos2| {
            let col = ((pos.x - rect.left()) / cell.x)
                .floor()
                .clamp(0.0, cols as f32 - 1.0) as usize;
            let row = ((pos.y - rect.top()) / cell.y)
                .floor()
                .clamp(0.0, rows as f32 - 1.0) as usize;
            (row, col)
        };
        let mut opened_link = false;
        if !blocked && ui.is_enabled() {
            if let Some(pos) = response.interact_pointer_pos() {
                let (row, col) = location(pos);
                if self.profile.hyperlinks_enabled
                    && response.clicked()
                    && ui.input(|i| i.modifiers.ctrl)
                    && let Some(url) = self.hyperlink_at(row, col)
                {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                    opened_link = true;
                }
                if !mouse_mode && !opened_link {
                    let point = viewport_to_point(
                        self.term.grid().display_offset(),
                        Point::new(row, Column(col)),
                    );
                    if response.triple_clicked() {
                        self.select_line(row);
                    } else if response.double_clicked() {
                        self.select_word(row, col);
                    } else if response.clicked() {
                        self.clear_selection();
                    }
                    if response.drag_started() {
                        // egui begins a drag after a threshold: anchor at press position, not current position.
                        let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(pos);
                        let (r, c) = location(origin);
                        let anchor = viewport_to_point(
                            self.term.grid().display_offset(),
                            Point::new(r, Column(c)),
                        );
                        self.term.selection =
                            Some(Selection::new(SelectionType::Simple, anchor, Side::Left));
                    }
                    if response.dragged()
                        && let Some(selection) = &mut self.term.selection
                    {
                        let side = if (pos.x - rect.left()).rem_euclid(cell.x) < cell.x / 2.0 {
                            Side::Left
                        } else {
                            Side::Right
                        };
                        selection.update(point, side);
                    }
                }
            }
            if self.profile.copy_on_select
                && (response.drag_stopped()
                    || response.double_clicked()
                    || response.triple_clicked())
                && let Some(text) = self.selection_text()
            {
                crate::set_clipboard_text(&text);
                ui.ctx().copy_text(text);
            }
            if response.hovered() && !mouse_mode {
                let scroll = ui.input(|i| i.raw_scroll_delta.y);
                if scroll != 0.0 {
                    self.term
                        .scroll_display(Scroll::Delta((scroll / cell.y).round() as i32));
                }
            }
            let hovered = ui
                .input(|i| i.pointer.hover_pos())
                .filter(|p| rect.contains(*p))
                .map(location);
            let link = if self.profile.hyperlinks_enabled {
                hovered.and_then(|(r, c)| self.hyperlink_at(r, c))
            } else {
                None
            };
            let path = hovered
                .and_then(|(r, c)| self.token_at(r, c))
                .filter(|p| p.starts_with('/') && !p.chars().any(char::is_control));
            if !mouse_mode {
                response.context_menu(|ui| {
                    if ui.button("Copy").clicked() {
                        if let Some(text) = self.selection_text() {
                            crate::set_clipboard_text(&text);
                            ui.ctx().copy_text(text);
                        }
                        ui.close();
                    }
                    if ui.button("Paste").clicked() {
                        if let Some(text) = crate::get_clipboard_text() {
                            if self.profile.multiline_paste_policy != MultilinePastePolicy::Off
                                && crate::paste_lines(&text) > 1
                            {
                                self.pending_paste = Some(text);
                            } else {
                                action.input.extend(encode_paste(
                                    &text,
                                    mode.contains(TermMode::BRACKETED_PASTE),
                                ));
                            }
                        } else {
                            self.paste_requested = true;
                            ui.ctx()
                                .send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                        }
                        response.request_focus();
                        ui.close();
                    }
                    if self.profile.hyperlinks_enabled
                        && let Some(url) = &link
                        && ui.button("Open link").clicked()
                    {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                        ui.close();
                    }
                    if sftp_available
                        && let Some(path) = &path
                        && ui.button("Reveal in SFTP").clicked()
                    {
                        action.reveal_path = Some(path.clone());
                        ui.close();
                    }
                });
            }
            for event in ui.input(|i| i.events.clone()) {
                if response.has_focus() && !opened_link && !self.search.focus {
                    let selected = self.term.selection.as_ref().is_some_and(|s| !s.is_empty());
                    let mut intent = clipboard_intent(
                        &event,
                        ui.input(|i| i.modifiers),
                        self.profile.clipboard_profile,
                        selected,
                        self.profile.literal_control_keys,
                    );
                    if matches!(event, Event::Paste(_)) && self.paste_requested {
                        self.paste_requested = false;
                        intent = Some(ClipboardIntent::Paste);
                    }
                    if let Some(intent) = intent {
                        match intent {
                            ClipboardIntent::Copy => {
                                if let Some(text) = self.selection_text() {
                                    crate::set_clipboard_text(&text);
                                    ui.ctx().copy_text(text);
                                }
                            }
                            ClipboardIntent::Control(byte) => action.input.push(byte),
                            ClipboardIntent::RequestPaste => {
                                if let Some(text) = crate::get_clipboard_text() {
                                    if self.profile.multiline_paste_policy
                                        != MultilinePastePolicy::Off
                                        && crate::paste_lines(&text) > 1
                                    {
                                        self.pending_paste = Some(text);
                                        break;
                                    } else {
                                        action.input.extend(encode_paste(
                                            &text,
                                            mode.contains(TermMode::BRACKETED_PASTE),
                                        ));
                                    }
                                } else {
                                    self.paste_requested = true;
                                    ui.ctx()
                                        .send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                                }
                            }
                            ClipboardIntent::Paste => {
                                if let Event::Paste(text) = event {
                                    if self.profile.multiline_paste_policy
                                        != MultilinePastePolicy::Off
                                        && crate::paste_lines(&text) > 1
                                    {
                                        self.pending_paste = Some(text);
                                        break;
                                    } else {
                                        action.input.extend(encode_paste(
                                            &text,
                                            mode.contains(TermMode::BRACKETED_PASTE),
                                        ));
                                    }
                                }
                            }
                        }
                        continue;
                    }
                    match &event {
                        Event::Text(text) => {
                            if ui.input(|i| i.modifiers.alt) {
                                action.input.push(27);
                            }
                            action.input.extend_from_slice(text.as_bytes());
                        }
                        Event::Key {
                            key,
                            pressed: true,
                            modifiers,
                            ..
                        } => {
                            if *key == Key::F
                                && modifiers.ctrl
                                && !modifiers.alt
                                && !modifiers.shift
                            {
                                self.search.open = true;
                                self.search.focus = true;
                                self.refresh_search();
                            } else if modifiers.shift && matches!(key, Key::PageUp | Key::PageDown)
                            {
                                self.term.scroll_display(if *key == Key::PageUp {
                                    Scroll::PageUp
                                } else {
                                    Scroll::PageDown
                                });
                            } else if let Some(bytes) =
                                encode_key(*key, *modifiers, mode.contains(TermMode::APP_CURSOR))
                            {
                                action.input.extend(bytes);
                            }
                        }
                        _ => {}
                    }
                }
                if mouse_mode && !opened_link && !(link.is_some() && ui.input(|i| i.modifiers.ctrl))
                {
                    match event {
                        Event::PointerButton {
                            pos,
                            button,
                            pressed,
                            modifiers,
                        } if rect.contains(pos) => {
                            let button = match button {
                                egui::PointerButton::Primary => 0,
                                egui::PointerButton::Middle => 1,
                                egui::PointerButton::Secondary => 2,
                                _ => continue,
                            };
                            mouse(
                                &mut action.input,
                                pos,
                                (rect, cell),
                                button,
                                pressed,
                                modifiers,
                                mode,
                            );
                        }
                        Event::PointerMoved(pos)
                            if rect.contains(pos)
                                && (mode.contains(TermMode::MOUSE_MOTION)
                                    || (mode.contains(TermMode::MOUSE_DRAG)
                                        && ui.input(|i| i.pointer.any_down()))) =>
                        {
                            let button = ui.input(|i| {
                                if i.pointer.primary_down() {
                                    0
                                } else if i.pointer.middle_down() {
                                    1
                                } else if i.pointer.secondary_down() {
                                    2
                                } else {
                                    3
                                }
                            });
                            mouse(
                                &mut action.input,
                                pos,
                                (rect, cell),
                                32 + button,
                                true,
                                ui.input(|i| i.modifiers),
                                mode,
                            );
                        }
                        Event::MouseWheel {
                            delta, modifiers, ..
                        } if response.hovered() && delta.y != 0.0 => {
                            if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                                mouse(
                                    &mut action.input,
                                    pos,
                                    (rect, cell),
                                    if delta.y > 0.0 { 64 } else { 65 },
                                    true,
                                    modifiers,
                                    mode,
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        if let Some(text) = &self.pending_paste {
            let mut confirm = false;
            let mut cancel = false;
            egui::Modal::new(response.id.with("paste-preview")).show(ui.ctx(),|ui| {
                ui.heading(format!("Paste {} lines?",crate::paste_lines(text)));
                let preview: String = text.chars().take(2000).collect();
                ui.monospace(preview.lines().take(8).collect::<Vec<_>>().join("\n"));
                if text.contains('\x1b') { ui.label("Escape characters are removed in bracketed paste to prevent ending the protected paste early."); }
                ui.horizontal(|ui| { cancel = ui.button("Cancel").clicked(); confirm = ui.button("Paste").clicked(); });
                cancel |= ui.input(|i| i.key_pressed(Key::Escape));
            });
            if confirm && ui.is_enabled() {
                self.restore_focus = true;
                if let Some(text) = self.pending_paste.take() {
                    action.input.extend(encode_paste(
                        &text,
                        mode.contains(TermMode::BRACKETED_PASTE),
                    ));
                }
                response.request_focus();
            }
            if cancel {
                self.pending_paste = None;
                self.restore_focus = true;
                response.request_focus();
            }
        }
        if !action.input.is_empty() {
            self.term.scroll_display(Scroll::Bottom);
            self.clear_selection();
        }
        action.input.extend(self.replies());
        if self
            .listener
            .1
            .swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            let now = ui.input(|i| i.time);
            self.bell_until = now + 0.15;
            if now >= self.audio_after {
                action.audio_bell = self.profile.bell_audio;
                self.audio_after = now + 0.5;
            }
        }
        if self.search.open && self.search.dirty {
            self.refresh_search();
        }
        self.paint(ui, rect, cell, response.has_focus() && !blocked);
        action
    }
}
fn mouse(
    out: &mut Vec<u8>,
    pos: egui::Pos2,
    geometry: (Rect, egui::Vec2),
    button: u8,
    pressed: bool,
    modifiers: egui::Modifiers,
    mode: TermMode,
) {
    let (rect, cell) = geometry;
    let x = ((pos.x - rect.left()) / cell.x) as u32 + 1;
    let y = ((pos.y - rect.top()) / cell.y) as u32 + 1;
    let code = button
        + 4 * u8::from(modifiers.shift)
        + 8 * u8::from(modifiers.alt)
        + 16 * u8::from(modifiers.ctrl);
    if mode.contains(TermMode::SGR_MOUSE) {
        out.extend_from_slice(
            format!("\x1b[<{code};{x};{y}{}", if pressed { 'M' } else { 'm' }).as_bytes(),
        );
    } else if x < 224 && y < 224 {
        out.extend_from_slice(&[
            27,
            b'[',
            b'M',
            32 + if pressed { code } else { 3 },
            32 + x as u8,
            32 + y as u8,
        ]);
    }
}
