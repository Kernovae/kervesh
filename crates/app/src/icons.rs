use crate::classify::FileType;
use egui::{Color32, Painter, Pos2, Rect, Stroke, Vec2, epaint::PathShape};

pub fn paint_monogram(painter: &Painter, rect: Rect, color: Color32) {
    let scale = rect.width().min(rect.height()) / 256.0;
    let origin = rect.min;

    let map = |x: f32, y: f32| -> Pos2 { Pos2::new(origin.x + x * scale, origin.y + y * scale) };

    // Left vertical stem: rect(34, 28, 50, 200, rx=7)
    let stem_rect = Rect::from_min_max(map(34.0, 28.0), map(84.0, 228.0));
    painter.rect_filled(stem_rect, 7.0 * scale, color);

    // Upper diagonal: M91 118 L158 40 H226 L141 126 H108 Z
    let upper = vec![
        map(91.0, 118.0),
        map(158.0, 40.0),
        map(226.0, 40.0),
        map(141.0, 126.0),
        map(108.0, 126.0),
    ];
    painter.add(PathShape::convex_polygon(upper, color, Stroke::NONE));

    // Lower diagonal: M108 130 H141 L226 216 H158 L91 138 Z
    let lower = vec![
        map(108.0, 130.0),
        map(141.0, 130.0),
        map(226.0, 216.0),
        map(158.0, 216.0),
        map(91.0, 138.0),
    ];
    painter.add(PathShape::convex_polygon(lower, color, Stroke::NONE));

    // Horizontal bridge: rect(79, 120, 43, 16, rx=8)
    let bridge_rect = Rect::from_min_max(map(79.0, 120.0), map(122.0, 136.0));
    painter.rect_filled(bridge_rect, 8.0 * scale, color);

    // Center dot: circle(91, 128, r=10)
    painter.circle_filled(map(91.0, 128.0), 10.0 * scale, color);
}

pub fn paint_file_icon(painter: &Painter, rect: Rect, file_type: FileType, color: Color32) {
    let scale = rect.width().min(rect.height()) / 16.0;
    let origin = rect.min;
    let stroke_w = (1.2 * scale).max(1.0);
    let stroke = Stroke::new(stroke_w, color);

    let map = |x: f32, y: f32| -> Pos2 { Pos2::new(origin.x + x * scale, origin.y + y * scale) };

    match file_type {
        FileType::Folder => {
            // Folder tab + body: M1.5 4 h4 l1.3 1.5 h7.7 v7.8 H1.5 z
            let points = vec![
                map(1.5, 4.0),
                map(5.5, 4.0),
                map(6.8, 5.5),
                map(14.5, 5.5),
                map(14.5, 13.3),
                map(1.5, 13.3),
            ];
            painter.add(PathShape::closed_line(points, stroke));
        }
        FileType::Key => {
            // Key circle + stem with teeth
            painter.circle_stroke(map(5.3, 7.0), 2.5 * scale, stroke);
            painter.line_segment([map(7.4, 8.2), map(12.4, 13.2)], stroke);
            painter.line_segment([map(10.1, 10.9), map(11.1, 9.9)], stroke);
            painter.line_segment([map(11.6, 12.4), map(12.6, 11.4)], stroke);
        }
        FileType::Database => {
            // Top ellipse + cylinder
            let ry = 2.0 * scale;
            let center = map(8.0, 4.0);
            painter.circle_stroke(center, ry, stroke);
            painter.line_segment([map(3.5, 4.0), map(3.5, 11.8)], stroke);
            painter.line_segment([map(12.5, 4.0), map(12.5, 11.8)], stroke);
            painter.line_segment([map(3.5, 11.8), map(12.5, 11.8)], stroke);
            painter.line_segment([map(3.5, 7.8), map(12.5, 7.8)], stroke);
        }
        _ => {
            // Base document page: M3 1.5 h6 l4 4 v9 H3 z + fold M9 1.5 v4 h4
            let page = vec![
                map(3.0, 1.5),
                map(9.0, 1.5),
                map(13.0, 5.5),
                map(13.0, 14.5),
                map(3.0, 14.5),
            ];
            painter.add(PathShape::closed_line(page, stroke));
            painter.line_segment([map(9.0, 1.5), map(9.0, 5.5)], stroke);
            painter.line_segment([map(9.0, 5.5), map(13.0, 5.5)], stroke);

            let inner_stroke = Stroke::new((0.95 * scale).max(0.8), color);

            // Specific document interior glyphs
            match file_type {
                FileType::GenericFile => {}
                FileType::Text => {
                    painter.line_segment([map(5.0, 8.0), map(11.0, 8.0)], inner_stroke);
                    painter.line_segment([map(5.0, 10.2), map(11.0, 10.2)], inner_stroke);
                    painter.line_segment([map(5.0, 12.4), map(9.4, 12.4)], inner_stroke);
                }
                FileType::Log => {
                    painter.line_segment([map(5.0, 8.0), map(11.0, 8.0)], inner_stroke);
                    painter.line_segment([map(5.0, 10.2), map(9.0, 10.2)], inner_stroke);
                    painter.line_segment([map(5.0, 12.4), map(10.0, 12.4)], inner_stroke);
                }
                FileType::Pdf => {
                    painter.line_segment([map(5.0, 8.0), map(11.0, 8.0)], inner_stroke);
                    painter.line_segment([map(5.0, 10.2), map(9.7, 10.2)], inner_stroke);
                    painter.line_segment([map(5.0, 12.4), map(8.7, 12.4)], inner_stroke);
                }
                FileType::Markdown => {
                    // M glyph
                    painter.line_segment([map(4.9, 8.2), map(4.9, 12.2)], inner_stroke);
                    painter.line_segment([map(4.9, 8.2), map(6.3, 9.8)], inner_stroke);
                    painter.line_segment([map(6.3, 9.8), map(7.7, 8.2)], inner_stroke);
                    painter.line_segment([map(7.7, 8.2), map(7.7, 12.2)], inner_stroke);
                    // Arrow
                    painter.line_segment([map(10.1, 8.6), map(10.1, 11.9)], inner_stroke);
                    painter.line_segment([map(9.2, 9.4), map(11.0, 9.4)], inner_stroke);
                }
                FileType::Code => {
                    // < and >
                    painter.line_segment([map(7.0, 8.0), map(5.2, 9.8)], inner_stroke);
                    painter.line_segment([map(5.2, 9.8), map(7.0, 11.5)], inner_stroke);
                    painter.line_segment([map(9.0, 8.0), map(10.8, 9.8)], inner_stroke);
                    painter.line_segment([map(10.8, 9.8), map(9.0, 11.5)], inner_stroke);
                }
                FileType::Shell => {
                    // > _
                    painter.line_segment([map(5.1, 8.4), map(6.9, 9.9)], inner_stroke);
                    painter.line_segment([map(6.9, 9.9), map(5.1, 11.4)], inner_stroke);
                    painter.line_segment([map(8.4, 11.4), map(11.0, 11.4)], inner_stroke);
                }
                FileType::Rust => {
                    // Gear circle with spokes
                    painter.circle_stroke(map(8.0, 10.1), 2.15 * scale, inner_stroke);
                    painter.line_segment([map(8.0, 7.0), map(8.0, 8.0)], inner_stroke);
                    painter.line_segment([map(8.0, 12.2), map(8.0, 13.2)], inner_stroke);
                    painter.line_segment([map(4.9, 10.1), map(5.9, 10.1)], inner_stroke);
                    painter.line_segment([map(10.1, 10.1), map(11.1, 10.1)], inner_stroke);
                }
                FileType::Config => {
                    // Settings gear
                    painter.circle_stroke(map(8.0, 10.1), 1.6 * scale, inner_stroke);
                    painter.line_segment([map(8.0, 7.5), map(8.0, 8.5)], inner_stroke);
                    painter.line_segment([map(8.0, 11.7), map(8.0, 12.7)], inner_stroke);
                    painter.line_segment([map(5.4, 10.1), map(6.4, 10.1)], inner_stroke);
                    painter.line_segment([map(9.6, 10.1), map(10.6, 10.1)], inner_stroke);
                }
                FileType::Json => {
                    // { and }
                    painter.line_segment([map(6.4, 7.8), map(5.5, 10.1)], inner_stroke);
                    painter.line_segment([map(5.5, 10.1), map(6.4, 12.4)], inner_stroke);
                    painter.line_segment([map(9.6, 7.8), map(10.5, 10.1)], inner_stroke);
                    painter.line_segment([map(10.5, 10.1), map(9.6, 12.4)], inner_stroke);
                }
                FileType::Image => {
                    // Sun circle + mountain lines
                    painter.circle_filled(map(6.1, 8.2), 0.9 * scale, color);
                    let mountain = vec![
                        map(4.5, 12.3),
                        map(6.8, 10.1),
                        map(8.3, 11.4),
                        map(9.8, 9.7),
                        map(11.5, 12.3),
                    ];
                    painter.add(PathShape::line(mountain, inner_stroke));
                }
                FileType::Archive => {
                    // Zipper lines
                    painter.line_segment([map(7.5, 7.5), map(7.5, 12.5)], inner_stroke);
                    painter.line_segment([map(6.5, 8.5), map(8.5, 8.5)], inner_stroke);
                    painter.line_segment([map(6.5, 10.5), map(8.5, 10.5)], inner_stroke);
                }
                FileType::Certificate => {
                    // Badge circle + ribbon
                    painter.circle_stroke(map(8.0, 9.4), 1.9 * scale, inner_stroke);
                    painter.line_segment([map(7.0, 11.0), map(6.5, 13.0)], inner_stroke);
                    painter.line_segment([map(6.5, 13.0), map(8.0, 12.2)], inner_stroke);
                    painter.line_segment([map(8.0, 12.2), map(9.5, 13.0)], inner_stroke);
                    painter.line_segment([map(9.5, 13.0), map(9.0, 11.0)], inner_stroke);
                }
                FileType::Executable => {
                    // Cross/star
                    painter.line_segment([map(5.0, 10.0), map(11.0, 10.0)], inner_stroke);
                    painter.line_segment([map(8.0, 7.0), map(8.0, 13.0)], inner_stroke);
                    painter.line_segment([map(6.1, 8.1), map(9.9, 11.9)], inner_stroke);
                    painter.line_segment([map(9.9, 8.1), map(6.1, 11.9)], inner_stroke);
                }
                FileType::Symlink => {
                    // Curved arrow
                    painter.line_segment([map(5.1, 12.0), map(5.1, 9.0)], inner_stroke);
                    painter.line_segment([map(5.1, 9.0), map(9.7, 9.0)], inner_stroke);
                    painter.line_segment([map(8.3, 7.5), map(10.3, 9.0)], inner_stroke);
                    painter.line_segment([map(8.3, 10.5), map(10.3, 9.0)], inner_stroke);
                }
                FileType::Folder | FileType::Key | FileType::Database => unreachable!(),
            }
        }
    }
}

pub fn render_file_icon(ui: &mut egui::Ui, file_type: FileType, color: Color32) -> egui::Response {
    let size = Vec2::splat(16.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_file_icon(ui.painter(), rect, file_type, color);
    }
    response
}

pub fn render_monogram(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_monogram(ui.painter(), rect, color);
    }
    response
}
