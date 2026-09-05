use egui::{Color32, Rect, Stroke};

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
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(left, cy), egui::pos2(cx, bottom)),
                    0.0,
                    fg,
                );
                return true;
            }
            '\u{2597}' => {
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(cx, cy), egui::pos2(right, bottom)),
                    0.0,
                    fg,
                );
                return true;
            }
            '\u{2598}' => {
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(left, top), egui::pos2(cx, cy)),
                    0.0,
                    fg,
                );
                return true;
            }
            '\u{2599}' => {
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
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(cx, top), egui::pos2(right, cy)),
                    0.0,
                    fg,
                );
                return true;
            }
            '\u{259E}' => {
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
