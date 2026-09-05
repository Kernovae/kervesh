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
                if is_custom_glyph(cell_data.c) && cell_data.zerowidth().is_none() {
                    draw_custom_glyph(&painter, cell_rect, cell_data.c, fg);
                } else {
                    painter.text(pos, Align2::LEFT_TOP, text, font.clone(), fg);
                }
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

pub fn is_custom_glyph(c: char) -> bool {
    matches!(
        c,
        '\u{2500}'..='\u{257F}' // Box Drawing
        | '\u{2580}'..='\u{259F}' // Block Elements
        | '\u{2190}'..='\u{2199}' // Arrows (←, ↑, →, ↓, ↔, ↕, ↖, ↗, ↘, ↙)
        | '\u{25B2}' | '\u{25BC}' | '\u{25C0}' | '\u{25B6}' // Triangles ▲ ▼ ◀ ▶
        | '\u{25C6}' | '\u{25CF}' | '\u{25CB}' // Diamond, filled/open circle ◆ ● ○
    )
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum LineWeight {
    None,
    Light,
    Heavy,
    Double,
}

pub fn draw_custom_glyph(painter: &egui::Painter, rect: Rect, c: char, fg: Color32) -> bool {
    let cx = rect.center().x;
    let cy = rect.center().y;
    let left = rect.left();
    let right = rect.right();
    let top = rect.top();
    let bottom = rect.bottom();
    let width = rect.width();
    let height = rect.height();

    let stroke_light = (height * 0.08).clamp(1.0, 2.0);
    let stroke_heavy = (stroke_light * 2.5).clamp(2.5, 4.0);

    // 1. Block Elements (U+2580 - U+259F)
    if ('\u{2580}'..='\u{259F}').contains(&c) {
        match c {
            // Upper half block
            '\u{2580}' => {
                painter.rect_filled(Rect::from_min_max(rect.min, egui::pos2(right, cy)), 0.0, fg);
                return true;
            }
            // Lower 1/8 to 7/8 blocks (U+2581..=U+2587)
            '\u{2581}'..='\u{2587}' => {
                let fraction = (c as u32 - 0x2580) as f32 / 8.0;
                let y = bottom - height * fraction;
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(left, y), egui::pos2(right, bottom)),
                    0.0,
                    fg,
                );
                return true;
            }
            // Full block
            '\u{2588}' => {
                painter.rect_filled(rect, 0.0, fg);
                return true;
            }
            // Left 7/8 to 1/8 blocks (U+2589..=U+258F)
            '\u{2589}'..='\u{258F}' => {
                let fraction = (8 - (c as u32 - 0x2588)) as f32 / 8.0;
                let x = left + width * fraction;
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(left, top), egui::pos2(x, bottom)),
                    0.0,
                    fg,
                );
                return true;
            }
            // Right half block
            '\u{2590}' => {
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(cx, top), egui::pos2(right, bottom)),
                    0.0,
                    fg,
                );
                return true;
            }
            // Light shade (25% stipple/opacity)
            '\u{2591}' => {
                painter.rect_filled(rect, 0.0, fg.gamma_multiply(0.25));
                return true;
            }
            // Medium shade (50% stipple/opacity)
            '\u{2592}' => {
                painter.rect_filled(rect, 0.0, fg.gamma_multiply(0.50));
                return true;
            }
            // Dark shade (75% stipple/opacity)
            '\u{2593}' => {
                painter.rect_filled(rect, 0.0, fg.gamma_multiply(0.75));
                return true;
            }
            // Upper 1/8 block
            '\u{2594}' => {
                painter.rect_filled(
                    Rect::from_min_max(rect.min, egui::pos2(right, top + height * 0.125)),
                    0.0,
                    fg,
                );
                return true;
            }
            // Right 1/8 block
            '\u{2595}' => {
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(right - width * 0.125, top), rect.max),
                    0.0,
                    fg,
                );
                return true;
            }
            // Quadrants (U+2596 - U+259F)
            '\u{2596}' => {
                // Lower-left
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(left, cy), egui::pos2(cx, bottom)),
                    0.0,
                    fg,
                );
                return true;
            }
            '\u{2597}' => {
                // Lower-right
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(cx, cy), egui::pos2(right, bottom)),
                    0.0,
                    fg,
                );
                return true;
            }
            '\u{2598}' => {
                // Upper-left
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(left, top), egui::pos2(cx, cy)),
                    0.0,
                    fg,
                );
                return true;
            }
            '\u{2599}' => {
                // Upper-left, lower-left, lower-right
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(left, top), egui::pos2(cx, cy)),
                    0.0,
                    fg,
                );
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(left, cy), egui::pos2(right, bottom)),
                    0.0,
                    fg,
                );
                return true;
            }
            '\u{259A}' => {
                // Upper-left and lower-right
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(left, top), egui::pos2(cx, cy)),
                    0.0,
                    fg,
                );
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(cx, cy), egui::pos2(right, bottom)),
                    0.0,
                    fg,
                );
                return true;
            }
            '\u{259B}' => {
                // Upper-left, upper-right, lower-left
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, cy)),
                    0.0,
                    fg,
                );
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(left, cy), egui::pos2(cx, bottom)),
                    0.0,
                    fg,
                );
                return true;
            }
            '\u{259C}' => {
                // Upper-left, upper-right, lower-right
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, cy)),
                    0.0,
                    fg,
                );
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(cx, cy), egui::pos2(right, bottom)),
                    0.0,
                    fg,
                );
                return true;
            }
            '\u{259D}' => {
                // Upper-right
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(cx, top), egui::pos2(right, cy)),
                    0.0,
                    fg,
                );
                return true;
            }
            '\u{259E}' => {
                // Upper-right and lower-left
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(cx, top), egui::pos2(right, cy)),
                    0.0,
                    fg,
                );
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(left, cy), egui::pos2(cx, bottom)),
                    0.0,
                    fg,
                );
                return true;
            }
            '\u{259F}' => {
                // Upper-right, lower-left, lower-right
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(cx, top), egui::pos2(right, cy)),
                    0.0,
                    fg,
                );
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(left, cy), egui::pos2(right, bottom)),
                    0.0,
                    fg,
                );
                return true;
            }
            _ => {}
        }
    }

    // 2. Common Arrows (U+2190 - U+2199)
    if ('\u{2190}'..='\u{2199}').contains(&c) {
        let stroke = Stroke::new(stroke_light * 1.2, fg);
        let arrow_size = (width.min(height) * 0.28).max(3.0);
        match c {
            '←' => {
                painter.line_segment(
                    [egui::pos2(right - 2.0, cy), egui::pos2(left + 2.0, cy)],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(left + 2.0 + arrow_size, cy - arrow_size),
                        egui::pos2(left + 2.0, cy),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(left + 2.0 + arrow_size, cy + arrow_size),
                        egui::pos2(left + 2.0, cy),
                    ],
                    stroke,
                );
                return true;
            }
            '→' => {
                painter.line_segment(
                    [egui::pos2(left + 2.0, cy), egui::pos2(right - 2.0, cy)],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(right - 2.0 - arrow_size, cy - arrow_size),
                        egui::pos2(right - 2.0, cy),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(right - 2.0 - arrow_size, cy + arrow_size),
                        egui::pos2(right - 2.0, cy),
                    ],
                    stroke,
                );
                return true;
            }
            '↑' => {
                painter.line_segment(
                    [egui::pos2(cx, bottom - 2.0), egui::pos2(cx, top + 2.0)],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(cx - arrow_size, top + 2.0 + arrow_size),
                        egui::pos2(cx, top + 2.0),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(cx + arrow_size, top + 2.0 + arrow_size),
                        egui::pos2(cx, top + 2.0),
                    ],
                    stroke,
                );
                return true;
            }
            '↓' => {
                painter.line_segment(
                    [egui::pos2(cx, top + 2.0), egui::pos2(cx, bottom - 2.0)],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(cx - arrow_size, bottom - 2.0 - arrow_size),
                        egui::pos2(cx, bottom - 2.0),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(cx + arrow_size, bottom - 2.0 - arrow_size),
                        egui::pos2(cx, bottom - 2.0),
                    ],
                    stroke,
                );
                return true;
            }
            '↔' => {
                painter.line_segment(
                    [egui::pos2(left + 2.0, cy), egui::pos2(right - 2.0, cy)],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(left + 2.0 + arrow_size, cy - arrow_size),
                        egui::pos2(left + 2.0, cy),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(left + 2.0 + arrow_size, cy + arrow_size),
                        egui::pos2(left + 2.0, cy),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(right - 2.0 - arrow_size, cy - arrow_size),
                        egui::pos2(right - 2.0, cy),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(right - 2.0 - arrow_size, cy + arrow_size),
                        egui::pos2(right - 2.0, cy),
                    ],
                    stroke,
                );
                return true;
            }
            '↕' => {
                painter.line_segment(
                    [egui::pos2(cx, top + 2.0), egui::pos2(cx, bottom - 2.0)],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(cx - arrow_size, top + 2.0 + arrow_size),
                        egui::pos2(cx, top + 2.0),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(cx + arrow_size, top + 2.0 + arrow_size),
                        egui::pos2(cx, top + 2.0),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(cx - arrow_size, bottom - 2.0 - arrow_size),
                        egui::pos2(cx, bottom - 2.0),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(cx + arrow_size, bottom - 2.0 - arrow_size),
                        egui::pos2(cx, bottom - 2.0),
                    ],
                    stroke,
                );
                return true;
            }
            _ => {}
        }
    }

    // 3. Triangles & Geometric symbols
    match c {
        '▲' => {
            let pts = vec![
                egui::pos2(cx, top + height * 0.2),
                egui::pos2(left + width * 0.15, bottom - height * 0.2),
                egui::pos2(right - width * 0.15, bottom - height * 0.2),
            ];
            painter.add(egui::epaint::PathShape::convex_polygon(
                pts,
                fg,
                Stroke::NONE,
            ));
            return true;
        }
        '▼' => {
            let pts = vec![
                egui::pos2(left + width * 0.15, top + height * 0.2),
                egui::pos2(right - width * 0.15, top + height * 0.2),
                egui::pos2(cx, bottom - height * 0.2),
            ];
            painter.add(egui::epaint::PathShape::convex_polygon(
                pts,
                fg,
                Stroke::NONE,
            ));
            return true;
        }
        '◀' => {
            let pts = vec![
                egui::pos2(left + width * 0.2, cy),
                egui::pos2(right - width * 0.2, top + height * 0.15),
                egui::pos2(right - width * 0.2, bottom - height * 0.15),
            ];
            painter.add(egui::epaint::PathShape::convex_polygon(
                pts,
                fg,
                Stroke::NONE,
            ));
            return true;
        }
        '▶' => {
            let pts = vec![
                egui::pos2(left + width * 0.2, top + height * 0.15),
                egui::pos2(right - width * 0.2, cy),
                egui::pos2(left + width * 0.2, bottom - height * 0.15),
            ];
            painter.add(egui::epaint::PathShape::convex_polygon(
                pts,
                fg,
                Stroke::NONE,
            ));
            return true;
        }
        '◆' => {
            let pts = vec![
                egui::pos2(cx, top + height * 0.15),
                egui::pos2(right - width * 0.15, cy),
                egui::pos2(cx, bottom - height * 0.15),
                egui::pos2(left + width * 0.15, cy),
            ];
            painter.add(egui::epaint::PathShape::convex_polygon(
                pts,
                fg,
                Stroke::NONE,
            ));
            return true;
        }
        '●' => {
            let radius = (width.min(height) * 0.35).max(2.0);
            painter.circle_filled(egui::pos2(cx, cy), radius, fg);
            return true;
        }
        '○' => {
            let radius = (width.min(height) * 0.35).max(2.0);
            painter.circle_stroke(egui::pos2(cx, cy), radius, Stroke::new(stroke_light, fg));
            return true;
        }
        _ => {}
    }

    // 4. Box Drawing (U+2500 - U+257F)
    if ('\u{2500}'..='\u{257F}').contains(&c) {
        // Handle rounded corners explicitly
        match c {
            '╭' => {
                let stroke = Stroke::new(stroke_light, fg);
                painter.line_segment([egui::pos2(cx, cy + 2.0), egui::pos2(cx, bottom)], stroke);
                painter.line_segment([egui::pos2(cx + 2.0, cy), egui::pos2(right, cy)], stroke);
                painter.line_segment([egui::pos2(cx, cy + 2.0), egui::pos2(cx + 2.0, cy)], stroke);
                return true;
            }
            '╮' => {
                let stroke = Stroke::new(stroke_light, fg);
                painter.line_segment([egui::pos2(cx, cy + 2.0), egui::pos2(cx, bottom)], stroke);
                painter.line_segment([egui::pos2(left, cy), egui::pos2(cx - 2.0, cy)], stroke);
                painter.line_segment([egui::pos2(cx - 2.0, cy), egui::pos2(cx, cy + 2.0)], stroke);
                return true;
            }
            '╯' => {
                let stroke = Stroke::new(stroke_light, fg);
                painter.line_segment([egui::pos2(cx, top), egui::pos2(cx, cy - 2.0)], stroke);
                painter.line_segment([egui::pos2(left, cy), egui::pos2(cx - 2.0, cy)], stroke);
                painter.line_segment([egui::pos2(cx - 2.0, cy), egui::pos2(cx, cy - 2.0)], stroke);
                return true;
            }
            '╰' => {
                let stroke = Stroke::new(stroke_light, fg);
                painter.line_segment([egui::pos2(cx, top), egui::pos2(cx, cy - 2.0)], stroke);
                painter.line_segment([egui::pos2(cx + 2.0, cy), egui::pos2(right, cy)], stroke);
                painter.line_segment([egui::pos2(cx, cy - 2.0), egui::pos2(cx + 2.0, cy)], stroke);
                return true;
            }
            '╱' => {
                let stroke = Stroke::new(stroke_light, fg);
                painter.line_segment([egui::pos2(left, bottom), egui::pos2(right, top)], stroke);
                return true;
            }
            '╲' => {
                let stroke = Stroke::new(stroke_light, fg);
                painter.line_segment([egui::pos2(left, top), egui::pos2(right, bottom)], stroke);
                return true;
            }
            '╳' => {
                let stroke = Stroke::new(stroke_light, fg);
                painter.line_segment([egui::pos2(left, bottom), egui::pos2(right, top)], stroke);
                painter.line_segment([egui::pos2(left, top), egui::pos2(right, bottom)], stroke);
                return true;
            }
            _ => {}
        }

        let (top_w, bottom_w, left_w, right_w) = decode_box_char(c);

        let draw_v_arm =
            |p: &egui::Painter, y_start: f32, y_end: f32, weight: LineWeight| match weight {
                LineWeight::None => {}
                LineWeight::Light => {
                    let w = stroke_light;
                    p.rect_filled(
                        Rect::from_min_max(
                            egui::pos2(cx - w * 0.5, y_start.min(y_end)),
                            egui::pos2(cx + w * 0.5, y_start.max(y_end)),
                        ),
                        0.0,
                        fg,
                    );
                }
                LineWeight::Heavy => {
                    let w = stroke_heavy;
                    p.rect_filled(
                        Rect::from_min_max(
                            egui::pos2(cx - w * 0.5, y_start.min(y_end)),
                            egui::pos2(cx + w * 0.5, y_start.max(y_end)),
                        ),
                        0.0,
                        fg,
                    );
                }
                LineWeight::Double => {
                    let offset = (width * 0.18).clamp(1.5, 3.5);
                    let w = stroke_light;
                    p.rect_filled(
                        Rect::from_min_max(
                            egui::pos2(cx - offset - w * 0.5, y_start.min(y_end)),
                            egui::pos2(cx - offset + w * 0.5, y_start.max(y_end)),
                        ),
                        0.0,
                        fg,
                    );
                    p.rect_filled(
                        Rect::from_min_max(
                            egui::pos2(cx + offset - w * 0.5, y_start.min(y_end)),
                            egui::pos2(cx + offset + w * 0.5, y_start.max(y_end)),
                        ),
                        0.0,
                        fg,
                    );
                }
            };

        let draw_h_arm =
            |p: &egui::Painter, x_start: f32, x_end: f32, weight: LineWeight| match weight {
                LineWeight::None => {}
                LineWeight::Light => {
                    let w = stroke_light;
                    p.rect_filled(
                        Rect::from_min_max(
                            egui::pos2(x_start.min(x_end), cy - w * 0.5),
                            egui::pos2(x_start.max(x_end), cy + w * 0.5),
                        ),
                        0.0,
                        fg,
                    );
                }
                LineWeight::Heavy => {
                    let w = stroke_heavy;
                    p.rect_filled(
                        Rect::from_min_max(
                            egui::pos2(x_start.min(x_end), cy - w * 0.5),
                            egui::pos2(x_start.max(x_end), cy + w * 0.5),
                        ),
                        0.0,
                        fg,
                    );
                }
                LineWeight::Double => {
                    let offset = (height * 0.18).clamp(1.5, 3.5);
                    let w = stroke_light;
                    p.rect_filled(
                        Rect::from_min_max(
                            egui::pos2(x_start.min(x_end), cy - offset - w * 0.5),
                            egui::pos2(x_start.max(x_end), cy - offset + w * 0.5),
                        ),
                        0.0,
                        fg,
                    );
                    p.rect_filled(
                        Rect::from_min_max(
                            egui::pos2(x_start.min(x_end), cy + offset - w * 0.5),
                            egui::pos2(x_start.max(x_end), cy + offset + w * 0.5),
                        ),
                        0.0,
                        fg,
                    );
                }
            };

        draw_v_arm(painter, top, cy, top_w);
        draw_v_arm(painter, cy, bottom, bottom_w);
        draw_h_arm(painter, left, cx, left_w);
        draw_h_arm(painter, cx, right, right_w);

        return true;
    }

    false
}

fn decode_box_char(c: char) -> (LineWeight, LineWeight, LineWeight, LineWeight) {
    use LineWeight::*;
    match c {
        // Straight lines
        '─' | '┄' | '┈' => (None, None, Light, Light),
        '━' | '┅' | '┉' => (None, None, Heavy, Heavy),
        '│' | '┆' | '┊' => (Light, Light, None, None),
        '┃' | '┇' | '┋' => (Heavy, Heavy, None, None),
        // Corners
        '┌' => (None, Light, None, Light),
        '┍' => (None, Light, None, Heavy),
        '┎' => (None, Heavy, None, Light),
        '┏' => (None, Heavy, None, Heavy),
        '┐' => (None, Light, Light, None),
        '┑' => (None, Light, Heavy, None),
        '┒' => (None, Heavy, Light, None),
        '┓' => (None, Heavy, Heavy, None),
        '└' => (Light, None, None, Light),
        '┕' => (Light, None, None, Heavy),
        '┖' => (Heavy, None, None, Light),
        '┗' => (Heavy, None, None, Heavy),
        '┘' => (Light, None, Light, None),
        '┙' => (Light, None, Heavy, None),
        '┚' => (Heavy, None, Light, None),
        '┛' => (Heavy, None, Heavy, None),
        // Tees
        '├' => (Light, Light, None, Light),
        '┝' => (Light, Light, None, Heavy),
        '┞' => (Heavy, Light, None, Light),
        '┟' => (Light, Heavy, None, Light),
        '┠' => (Heavy, Heavy, None, Light),
        '┡' => (Heavy, Light, None, Heavy),
        '┢' => (Light, Heavy, None, Heavy),
        '┣' => (Heavy, Heavy, None, Heavy),
        '┤' => (Light, Light, Light, None),
        '┥' => (Light, Light, Heavy, None),
        '┦' => (Heavy, Light, Light, None),
        '┧' => (Light, Heavy, Light, None),
        '┨' => (Heavy, Heavy, Light, None),
        '┩' => (Heavy, Light, Heavy, None),
        '┪' => (Light, Heavy, Heavy, None),
        '┫' => (Heavy, Heavy, Heavy, None),
        '┬' => (None, Light, Light, Light),
        '┭' => (None, Light, Heavy, Light),
        '┮' => (None, Light, Light, Heavy),
        '┯' => (None, Light, Heavy, Heavy),
        '┰' => (None, Heavy, Light, Light),
        '┱' => (None, Heavy, Heavy, Light),
        '┲' => (None, Heavy, Light, Heavy),
        '┳' => (None, Heavy, Heavy, Heavy),
        '┴' => (Light, None, Light, Light),
        '┵' => (Light, None, Heavy, Light),
        '┶' => (Light, None, Light, Heavy),
        '┷' => (Light, None, Heavy, Heavy),
        '┸' => (Heavy, None, Light, Light),
        '┹' => (Heavy, None, Heavy, Light),
        '┺' => (Heavy, None, Light, Heavy),
        '┻' => (Heavy, None, Heavy, Heavy),
        // Crosses
        '┼' => (Light, Light, Light, Light),
        '┽' => (Light, Light, Heavy, Light),
        '┾' => (Light, Light, Light, Heavy),
        '┿' => (Light, Light, Heavy, Heavy),
        '╀' => (Heavy, Light, Light, Light),
        '╁' => (Light, Heavy, Light, Light),
        '╂' => (Heavy, Heavy, Light, Light),
        '╃' => (Heavy, Light, Heavy, Light),
        '╄' => (Heavy, Light, Light, Heavy),
        '╅' => (Light, Heavy, Heavy, Light),
        '╆' => (Light, Heavy, Light, Heavy),
        '╇' => (Heavy, Heavy, Heavy, Light),
        '╈' => (Heavy, Heavy, Light, Heavy),
        '╉' => (Heavy, Light, Heavy, Heavy),
        '╊' => (Light, Heavy, Heavy, Heavy),
        '╋' => (Heavy, Heavy, Heavy, Heavy),
        // Double lines and mixed double/single
        '═' => (None, None, Double, Double),
        '║' => (Double, Double, None, None),
        '╒' => (None, Light, None, Double),
        '╓' => (None, Double, None, Light),
        '╔' => (None, Double, None, Double),
        '╕' => (None, Light, Double, None),
        '╖' => (None, Double, Light, None),
        '╗' => (None, Double, Double, None),
        '╘' => (Light, None, None, Double),
        '╙' => (Double, None, None, Light),
        '╚' => (Double, None, None, Double),
        '╛' => (Light, None, Double, None),
        '╜' => (Double, None, Light, None),
        '╝' => (Double, None, Double, None),
        '╞' => (Light, Light, None, Double),
        '╟' => (Double, Double, None, Light),
        '╠' => (Double, Double, None, Double),
        '╡' => (Light, Light, Double, None),
        '╢' => (Double, Double, Light, None),
        '╣' => (Double, Double, Double, None),
        '╤' => (None, Light, Double, Double),
        '╥' => (None, Double, Light, Light),
        '╦' => (None, Double, Double, Double),
        '╧' => (Light, None, Double, Double),
        '╨' => (Double, None, Light, Light),
        '╩' => (Double, None, Double, Double),
        '╪' => (Light, Light, Double, Double),
        '╫' => (Double, Double, Light, Light),
        '╬' => (Double, Double, Double, Double),
        // Rays & half lines
        '╴' => (None, None, Light, None),
        '╵' => (Light, None, None, None),
        '╶' => (None, None, None, Light),
        '╷' => (None, Light, None, None),
        '╸' => (None, None, Heavy, None),
        '╹' => (Heavy, None, None, None),
        '╺' => (None, None, None, Heavy),
        '╻' => (None, Heavy, None, None),
        '╼' => (None, None, Light, Heavy),
        '╽' => (Light, Heavy, None, None),
        '╾' => (None, None, Heavy, Light),
        '╿' => (Heavy, Light, None, None),
        _ => (None, None, None, None),
    }
}
