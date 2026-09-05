use crate::{Terminal, TerminalPalette};
use alacritty_terminal::{
    term::{TermMode, cell::Flags, point_to_viewport},
    vte::ansi::{Color, CursorShape, NamedColor},
};
use egui::{Align2, Color32, Rect, Stroke, StrokeKind, Ui, vec2};

pub(crate) fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}
pub(crate) fn color(value: Color, palette: &TerminalPalette) -> Color32 {
    match value {
        Color::Spec(c) => Color32::from_rgb(c.r, c.g, c.b),
        Color::Named(NamedColor::Foreground | NamedColor::BrightForeground) => {
            rgb(palette.foreground)
        }
        Color::Named(NamedColor::DimForeground) => rgb(palette.foreground).gamma_multiply(0.65),
        Color::Named(NamedColor::Background) => rgb(palette.background),
        Color::Named(NamedColor::Cursor) => rgb(palette.cursor),
        Color::Named(named) => indexed(named as usize, palette),
        Color::Indexed(index) => indexed(index as usize, palette),
    }
}
fn indexed(index: usize, palette: &TerminalPalette) -> Color32 {
    if index < 16 {
        return rgb(palette.ansi[index]);
    }
    if (259..267).contains(&index) {
        return rgb(palette.ansi[index - 259]).gamma_multiply(0.65);
    }
    if index < 232 {
        let i = index - 16;
        let component = |v: usize| if v == 0 { 0 } else { (55 + 40 * v) as u8 };
        Color32::from_rgb(component(i / 36), component((i / 6) % 6), component(i % 6))
    } else if index < 256 {
        Color32::from_gray((8 + (index - 232) * 10) as u8)
    } else {
        rgb(palette.foreground)
    }
}
impl Terminal {
    pub(crate) fn paint(&self, ui: &Ui, rect: Rect, cell: egui::Vec2, focused: bool) {
        let painter = ui.painter_at(rect);
        let palette = &self.profile.palette;
        let base = rgb(palette.background);
        painter.rect_filled(rect, 0.0, base);
        let font = self.font_config.font_id(self.profile.font_size, false);
        let bold_font = self.font_config.font_id(self.profile.font_size, true);
        let y_offset = ui.fonts_mut(|f| (cell.y - f.row_height(&font)) / 2.0);
        let content = self.term.renderable_content();
        let resolve = |c: Color| {
            let index = match c {
                Color::Named(n) => Some(n as usize),
                Color::Indexed(n) => Some(n as usize),
                _ => None,
            };
            index
                .and_then(|i| content.colors[i])
                .map(|c| Color32::from_rgb(c.r, c.g, c.b))
                .unwrap_or_else(|| color(c, palette))
        };
        for indexed in content.display_iter {
            let Some(point) = point_to_viewport(content.display_offset, indexed.point) else {
                continue;
            };
            if point.line >= self.rows {
                continue;
            }
            let data = indexed.cell;
            let pos = rect.min + vec2(point.column.0 as f32 * cell.x, point.line as f32 * cell.y);
            let cell_rect = Rect::from_min_size(pos, cell);
            let mut fg = resolve(data.fg);
            let mut bg = resolve(data.bg);
            if data.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            if data.flags.contains(Flags::DIM) {
                fg = fg.gamma_multiply(0.65);
            }
            if content
                .selection
                .as_ref()
                .is_some_and(|s| s.contains(indexed.point))
            {
                bg = rgb(palette.selection);
            }
            if self.search.open {
                // Matches are sorted by grid position. No scan through scrollback per cell.
                let idx = self
                    .search
                    .matches
                    .partition_point(|m| m.end < indexed.point);
                if self
                    .search
                    .matches
                    .get(idx)
                    .is_some_and(|m| m.start <= indexed.point)
                {
                    bg = if idx == self.search.current {
                        Color32::from_rgb(210, 155, 40)
                    } else {
                        Color32::from_rgb(120, 105, 40)
                    };
                    fg = Color32::BLACK;
                }
            }
            if bg != base {
                painter.rect_filled(cell_rect, 0.0, bg);
            }
            if data.flags.intersects(
                Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER | Flags::HIDDEN,
            ) {
                continue;
            }
            let font = if data.flags.contains(Flags::BOLD) {
                &bold_font
            } else {
                &font
            };
            let mut buffer = [0; 4];
            let mut combined;
            let text = if let Some(marks) = data.zerowidth() {
                combined = data.c.to_string();
                combined.extend(marks);
                combined.as_str()
            } else {
                data.c.encode_utf8(&mut buffer)
            };
            // Every cell has fixed advance; wide glyphs can occupy two cells.
            let width = if data.flags.contains(Flags::WIDE_CHAR) {
                cell.x * 2.0
            } else {
                cell.x
            };
            if crate::is_custom_glyph(data.c) && data.zerowidth().is_none() {
                crate::draw_custom_glyph(&painter, cell_rect, data.c, fg);
            } else {
                painter
                    .with_clip_rect(Rect::from_min_size(pos, vec2(width, cell.y)).intersect(rect))
                    .text(
                        pos + vec2(0.0, y_offset),
                        Align2::LEFT_TOP,
                        text,
                        font.clone(),
                        fg,
                    );
            }
            if data.flags.intersects(Flags::ALL_UNDERLINES) {
                painter.line_segment(
                    [
                        cell_rect.left_bottom() - vec2(0.0, 1.0),
                        cell_rect.right_bottom() - vec2(0.0, 1.0),
                    ],
                    Stroke::new(1.0_f32, fg),
                );
            }
            if data.flags.contains(Flags::STRIKEOUT) {
                painter.line_segment(
                    [cell_rect.left_center(), cell_rect.right_center()],
                    Stroke::new(1.0_f32, fg),
                );
            }
        }
        let style = self.term.cursor_style();
        let time = ui.input(|i| i.time);
        let blink = focused && style.blinking;
        if content.mode.contains(TermMode::SHOW_CURSOR) && content.display_offset == 0 {
            if blink {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(
                        500 - ((time * 1000.0) as u64 % 500),
                    ));
            }
            if !blink || ((time * 2.0) as u64).is_multiple_of(2) {
                let point = content.cursor.point;
                let r = Rect::from_min_size(
                    rect.min + vec2(point.column.0 as f32 * cell.x, point.line.0 as f32 * cell.y),
                    cell,
                );
                let c = rgb(palette.cursor);
                if !focused {
                    painter.rect_stroke(r, 0.0, Stroke::new(1.0_f32, c), StrokeKind::Inside);
                } else {
                    match content.cursor.shape {
                        CursorShape::Beam => {
                            painter.rect_filled(
                                Rect::from_min_size(r.min, vec2(2.0, cell.y)),
                                0.0,
                                c,
                            );
                        }
                        CursorShape::Underline => {
                            painter.rect_filled(
                                Rect::from_min_size(
                                    r.left_bottom() - vec2(0.0, 2.0),
                                    vec2(cell.x, 2.0),
                                ),
                                0.0,
                                c,
                            );
                        }
                        CursorShape::Hidden => {}
                        _ => {
                            painter.rect_filled(r, 0.0, c.gamma_multiply(0.4));
                            painter.rect_stroke(
                                r,
                                0.0,
                                Stroke::new(1.0_f32, c),
                                StrokeKind::Inside,
                            );
                        }
                    }
                }
            }
        }
        if self.bell_until > time && self.profile.bell_visual {
            painter.rect_stroke(
                rect,
                0.0,
                Stroke::new(3.0_f32, rgb(palette.cursor)),
                StrokeKind::Inside,
            );
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs_f64(self.bell_until - time));
        }
    }
}
