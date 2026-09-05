use crate::{Terminal, TerminalAction, encode_key, encode_paste};
use alacritty_terminal::{
    grid::Scroll,
    index::{Column, Point, Side},
    selection::{Selection, SelectionType},
    term::{TermMode, cell::Flags, point_to_viewport, viewport_to_point},
    vte::ansi::{Color, NamedColor},
};
use egui::{Align2, Color32, Event, FontId, Key, Rect, Sense, Stroke, StrokeKind, Ui, vec2};

impl Terminal {
    pub fn ui(&mut self, ui: &mut Ui, font_size: f32) -> TerminalAction {
        let font = FontId::monospace(font_size);
        let cell = ui.fonts_mut(|f| vec2(f.glyph_width(&font, 'M'), f.row_height(&font)));
        let size = ui.available_size().max(vec2(cell.x * 2.0, cell.y));
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let mut action = TerminalAction::default();
        let cols = (rect.width() / cell.x).floor() as usize;
        let rows = (rect.height() / cell.y).floor() as usize;
        if (cols.max(2), rows.max(1)) != (self.cols, self.rows) {
            self.resize(cols, rows);
            action.resize = Some((self.cols as u32, self.rows as u32));
        }
        if response.clicked() || response.drag_started() {
            response.request_focus();
        }
        let mode = *self.term.mode();
        let mouse_mode = mode.intersects(TermMode::MOUSE_MODE) && !ui.input(|i| i.modifiers.shift);
        if let Some(pos) = response.interact_pointer_pos() {
            let col = ((pos.x - rect.left()) / cell.x)
                .floor()
                .clamp(0.0, self.cols as f32 - 1.0) as usize;
            let row = ((pos.y - rect.top()) / cell.y)
                .floor()
                .clamp(0.0, self.rows as f32 - 1.0) as usize;
            if !mouse_mode {
                let point = viewport_to_point(
                    self.term.grid().display_offset(),
                    Point::new(row, Column(col)),
                );
                if response.drag_started() {
                    self.term.selection =
                        Some(Selection::new(SelectionType::Simple, point, Side::Left));
                }
                if response.dragged()
                    && let Some(selection) = &mut self.term.selection
                {
                    selection.update(point, Side::Right);
                }
                if response.double_clicked() {
                    self.term.selection =
                        Some(Selection::new(SelectionType::Semantic, point, Side::Left));
                }
            }
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 && !mouse_mode {
                self.term
                    .scroll_display(Scroll::Delta((scroll / cell.y).round() as i32));
            }
        }
        if ui.is_enabled() && (response.has_focus() || (mouse_mode && response.hovered())) {
            for event in ui.input(|i| i.events.clone()) {
                if let Some(byte) = crate::clipboard_control(&event, ui.input(|i| i.modifiers)) {
                    action.input.push(byte);
                    continue;
                }
                match event {
                    Event::Copy => {
                        if let Some(text) = self.term.selection_to_string() {
                            ui.ctx().copy_text(text);
                        }
                    }
                    Event::Key {
                        key: Key::C,
                        pressed: true,
                        modifiers,
                        ..
                    } if modifiers.ctrl && modifiers.shift => {
                        if let Some(text) = self.term.selection_to_string() {
                            ui.ctx().copy_text(text);
                        }
                    }
                    Event::Paste(text) => action.input.extend(encode_paste(
                        &text,
                        mode.contains(TermMode::BRACKETED_PASTE),
                    )),
                    Event::Text(text) if response.has_focus() => {
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
                    } if response.has_focus() => {
                        if modifiers.shift && matches!(key, Key::PageUp | Key::PageDown) {
                            self.term.scroll_display(if key == Key::PageUp {
                                Scroll::PageUp
                            } else {
                                Scroll::PageDown
                            });
                        } else if let Some(bytes) =
                            encode_key(key, modifiers, mode.contains(TermMode::APP_CURSOR))
                        {
                            action.input.extend(bytes);
                        }
                    }
                    Event::PointerButton {
                        pos,
                        button,
                        pressed,
                        modifiers,
                    } if mouse_mode && rect.contains(pos) => {
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
                        if mouse_mode
                            && rect.contains(pos)
                            && (mode.contains(TermMode::MOUSE_MOTION)
                                || (mode.contains(TermMode::MOUSE_DRAG)
                                    && ui.input(|i| i.pointer.any_down()))) =>
                    {
                        let button = ui.input(|i| {
                            if i.pointer.primary_down() {
                                0
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
                    } if mouse_mode && response.hovered() => {
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
        if !action.input.is_empty() {
            self.term.scroll_display(Scroll::Bottom);
        }
        action.input.extend(self.replies());
        let painter = ui.painter_at(rect);
        let dark = ui.visuals().dark_mode;
        let base = if dark {
            Color32::from_rgb(17, 21, 25)
        } else {
            Color32::from_rgb(250, 250, 247)
        };
        painter.rect_filled(rect, 0.0, base);
        let content = self.term.renderable_content();
        let cursor = content.cursor.point;
        for indexed in content.display_iter {
            let Some(point) = point_to_viewport(content.display_offset, indexed.point) else {
                continue;
            };
            if point.line >= self.rows {
                continue;
            }
            let cell_data = indexed.cell;
            let pos = rect.min + vec2(point.column.0 as f32 * cell.x, point.line as f32 * cell.y);
            let cell_rect = Rect::from_min_size(pos, cell);
            let mut fg = color(cell_data.fg, dark);
            let mut bg = color(cell_data.bg, dark);
            if cell_data.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            if content
                .selection
                .as_ref()
                .is_some_and(|s| s.contains(indexed.point))
            {
                bg = Color32::from_rgb(44, 76, 96);
                fg = Color32::WHITE;
            }
            if bg != base {
                painter.rect_filled(cell_rect, 0.0, bg);
            }
            if !cell_data.flags.intersects(
                Flags::WIDE_CHAR_SPACER | Flags::HIDDEN | Flags::LEADING_WIDE_CHAR_SPACER,
            ) {
                let mut text = cell_data.c.to_string();
                if let Some(chars) = cell_data.zerowidth() {
                    text.extend(chars);
                }
                if cell_data.flags.contains(Flags::DIM) {
                    fg = fg.gamma_multiply(0.65);
                }
                painter.text(pos, Align2::LEFT_TOP, text, font.clone(), fg);
                if cell_data.flags.intersects(Flags::ALL_UNDERLINES) {
                    painter.line_segment(
                        [cell_rect.left_bottom(), cell_rect.right_bottom()],
                        Stroke::new(1.0_f32, fg),
                    );
                }
            }
            if indexed.point == cursor
                && content.mode.contains(TermMode::SHOW_CURSOR)
                && content.display_offset == 0
            {
                painter.rect_stroke(
                    cell_rect,
                    0.0,
                    Stroke::new(
                        if response.has_focus() {
                            1.5_f32
                        } else {
                            0.7_f32
                        },
                        Color32::from_rgb(100, 202, 160),
                    ),
                    StrokeKind::Inside,
                );
            }
        }
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
fn color(value: Color, dark: bool) -> Color32 {
    match value {
        Color::Spec(rgb) => Color32::from_rgb(rgb.r, rgb.g, rgb.b),
        Color::Named(NamedColor::Foreground) => {
            if dark {
                Color32::from_rgb(215, 223, 225)
            } else {
                Color32::from_rgb(30, 38, 42)
            }
        }
        Color::Named(NamedColor::Background) => {
            if dark {
                Color32::from_rgb(17, 21, 25)
            } else {
                Color32::from_rgb(250, 250, 247)
            }
        }
        Color::Named(named) => indexed(named as usize),
        Color::Indexed(index) => indexed(index as usize),
    }
}
fn indexed(index: usize) -> Color32 {
    const ANSI: [[u8; 3]; 16] = [
        [26, 30, 35],
        [215, 90, 90],
        [102, 190, 140],
        [220, 180, 100],
        [108, 160, 220],
        [180, 134, 209],
        [95, 190, 190],
        [210, 216, 220],
        [95, 105, 115],
        [245, 118, 118],
        [140, 220, 160],
        [240, 210, 140],
        [145, 185, 240],
        [205, 165, 235],
        [140, 220, 220],
        [245, 248, 250],
    ];
    let rgb = if index < 16 {
        ANSI[index]
    } else if index < 232 {
        let i = index - 16;
        let v = |c: usize| if c == 0 { 0 } else { 55 + c as u8 * 40 };
        [v(i / 36), v(i / 6 % 6), v(i % 6)]
    } else if index < 256 {
        [8 + (index as u8 - 232) * 10; 3]
    } else {
        [215, 223, 225]
    };
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}
