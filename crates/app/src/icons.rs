use crate::classify::FileType;
use egui::{
    Color32, ColorImage, Context, Painter, Rect, Response, Sense, TextureHandle, TextureOptions,
    Ui, Vec2,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum UiIcon {
    NewConnection,
    Split,
    Sftp,
    Monitor,
    Settings,
    Back,
    Forward,
    Parent,
    Refresh,
    Upload,
    NewFile,
    NewFolder,
    Download,
    Copy,
    Rename,
    Permissions,
    Delete,
    Pause,
    Cancel,
    Retry,
    Terminal,
    Host,
    Hosts,
    Inspector,
    Search,
    Bookmark,
    Connect,
    Disconnect,
    Files,
    Transfer,
}

impl UiIcon {
    fn dark_png_bytes(self) -> &'static [u8] {
        match self {
            Self::NewConnection => {
                include_bytes!("../../../assets/ui-icons/png/dark/20/new-connection.png")
            }
            Self::Split => include_bytes!("../../../assets/ui-icons/png/dark/20/split.png"),
            Self::Sftp => include_bytes!("../../../assets/ui-icons/png/dark/20/sftp.png"),
            Self::Monitor => include_bytes!("../../../assets/ui-icons/png/dark/20/monitor.png"),
            Self::Settings => include_bytes!("../../../assets/ui-icons/png/dark/20/settings.png"),
            Self::Back => include_bytes!("../../../assets/ui-icons/png/dark/16/back.png"),
            Self::Forward => include_bytes!("../../../assets/ui-icons/png/dark/16/forward.png"),
            Self::Parent => include_bytes!("../../../assets/ui-icons/png/dark/16/up.png"),
            Self::Refresh => include_bytes!("../../../assets/ui-icons/png/dark/16/refresh.png"),
            Self::Upload => include_bytes!("../../../assets/ui-icons/png/dark/16/upload.png"),
            Self::NewFile => include_bytes!("../../../assets/ui-icons/png/dark/16/new-file.png"),
            Self::NewFolder => {
                include_bytes!("../../../assets/ui-icons/png/dark/16/new-folder.png")
            }
            Self::Download => include_bytes!("../../../assets/ui-icons/png/dark/16/download.png"),
            Self::Copy => include_bytes!("../../../assets/ui-icons/png/dark/16/copy.png"),
            Self::Rename => include_bytes!("../../../assets/ui-icons/png/dark/16/rename.png"),
            Self::Permissions => {
                include_bytes!("../../../assets/ui-icons/png/dark/16/permissions.png")
            }
            Self::Delete => include_bytes!("../../../assets/ui-icons/png/dark/16/delete.png"),
            Self::Pause => include_bytes!("../../../assets/ui-icons/png/dark/16/pause.png"),
            Self::Cancel => include_bytes!("../../../assets/ui-icons/png/dark/16/cancel.png"),
            Self::Retry => include_bytes!("../../../assets/ui-icons/png/dark/16/retry.png"),
            Self::Terminal => include_bytes!("../../../assets/ui-icons/png/dark/16/terminal.png"),
            Self::Host => include_bytes!("../../../assets/ui-icons/png/dark/16/host.png"),
            Self::Hosts => include_bytes!("../../../assets/ui-icons/png/dark/16/hosts.png"),
            Self::Inspector => include_bytes!("../../../assets/ui-icons/png/dark/16/inspector.png"),
            Self::Search => include_bytes!("../../../assets/ui-icons/png/dark/16/search.png"),
            Self::Bookmark => include_bytes!("../../../assets/ui-icons/png/dark/16/bookmark.png"),
            Self::Connect => include_bytes!("../../../assets/ui-icons/png/dark/16/connect.png"),
            Self::Disconnect => {
                include_bytes!("../../../assets/ui-icons/png/dark/16/disconnect.png")
            }
            Self::Files => include_bytes!("../../../assets/ui-icons/png/dark/16/files.png"),
            Self::Transfer => include_bytes!("../../../assets/ui-icons/png/dark/16/transfer.png"),
        }
    }

    fn light_png_bytes(self) -> &'static [u8] {
        match self {
            Self::NewConnection => {
                include_bytes!("../../../assets/ui-icons/png/light/20/new-connection.png")
            }
            Self::Split => include_bytes!("../../../assets/ui-icons/png/light/20/split.png"),
            Self::Sftp => include_bytes!("../../../assets/ui-icons/png/light/20/sftp.png"),
            Self::Monitor => include_bytes!("../../../assets/ui-icons/png/light/20/monitor.png"),
            Self::Settings => include_bytes!("../../../assets/ui-icons/png/light/20/settings.png"),
            Self::Back => include_bytes!("../../../assets/ui-icons/png/light/16/back.png"),
            Self::Forward => include_bytes!("../../../assets/ui-icons/png/light/16/forward.png"),
            Self::Parent => include_bytes!("../../../assets/ui-icons/png/light/16/up.png"),
            Self::Refresh => include_bytes!("../../../assets/ui-icons/png/light/16/refresh.png"),
            Self::Upload => include_bytes!("../../../assets/ui-icons/png/light/16/upload.png"),
            Self::NewFile => include_bytes!("../../../assets/ui-icons/png/light/16/new-file.png"),
            Self::NewFolder => {
                include_bytes!("../../../assets/ui-icons/png/light/16/new-folder.png")
            }
            Self::Download => include_bytes!("../../../assets/ui-icons/png/light/16/download.png"),
            Self::Copy => include_bytes!("../../../assets/ui-icons/png/light/16/copy.png"),
            Self::Rename => include_bytes!("../../../assets/ui-icons/png/light/16/rename.png"),
            Self::Permissions => {
                include_bytes!("../../../assets/ui-icons/png/light/16/permissions.png")
            }
            Self::Delete => include_bytes!("../../../assets/ui-icons/png/light/16/delete.png"),
            Self::Pause => include_bytes!("../../../assets/ui-icons/png/light/16/pause.png"),
            Self::Cancel => include_bytes!("../../../assets/ui-icons/png/light/16/cancel.png"),
            Self::Retry => include_bytes!("../../../assets/ui-icons/png/light/16/retry.png"),
            Self::Terminal => include_bytes!("../../../assets/ui-icons/png/light/16/terminal.png"),
            Self::Host => include_bytes!("../../../assets/ui-icons/png/light/16/host.png"),
            Self::Hosts => include_bytes!("../../../assets/ui-icons/png/light/16/hosts.png"),
            Self::Inspector => {
                include_bytes!("../../../assets/ui-icons/png/light/16/inspector.png")
            }
            Self::Search => include_bytes!("../../../assets/ui-icons/png/light/16/search.png"),
            Self::Bookmark => include_bytes!("../../../assets/ui-icons/png/light/16/bookmark.png"),
            Self::Connect => include_bytes!("../../../assets/ui-icons/png/light/16/connect.png"),
            Self::Disconnect => {
                include_bytes!("../../../assets/ui-icons/png/light/16/disconnect.png")
            }
            Self::Files => include_bytes!("../../../assets/ui-icons/png/light/16/files.png"),
            Self::Transfer => include_bytes!("../../../assets/ui-icons/png/light/16/transfer.png"),
        }
    }
}

impl FileType {
    fn dark_png_bytes(self) -> &'static [u8] {
        match self {
            Self::Folder => include_bytes!("../../../assets/file-types/png/dark/16/folder.png"),
            Self::GenericFile => include_bytes!("../../../assets/file-types/png/dark/16/file.png"),
            Self::Pdf => include_bytes!("../../../assets/file-types/png/dark/16/pdf.png"),
            Self::Text => include_bytes!("../../../assets/file-types/png/dark/16/text.png"),
            Self::Markdown => include_bytes!("../../../assets/file-types/png/dark/16/markdown.png"),
            Self::Rust => include_bytes!("../../../assets/file-types/png/dark/16/rust.png"),
            Self::Shell => include_bytes!("../../../assets/file-types/png/dark/16/shell.png"),
            Self::Config => include_bytes!("../../../assets/file-types/png/dark/16/config.png"),
            Self::Json => include_bytes!("../../../assets/file-types/png/dark/16/json.png"),
            Self::Image => include_bytes!("../../../assets/file-types/png/dark/16/image.png"),
            Self::Archive => include_bytes!("../../../assets/file-types/png/dark/16/archive.png"),
            Self::Database => include_bytes!("../../../assets/file-types/png/dark/16/database.png"),
            Self::Key => include_bytes!("../../../assets/file-types/png/dark/16/key.png"),
            Self::Certificate => {
                include_bytes!("../../../assets/file-types/png/dark/16/certificate.png")
            }
            Self::Executable => {
                include_bytes!("../../../assets/file-types/png/dark/16/executable.png")
            }
            Self::Code => include_bytes!("../../../assets/file-types/png/dark/16/rust.png"),
            Self::Log => include_bytes!("../../../assets/file-types/png/dark/16/log.png"),
            Self::Symlink => include_bytes!("../../../assets/file-types/png/dark/16/symlink.png"),
        }
    }

    fn light_png_bytes(self) -> &'static [u8] {
        match self {
            Self::Folder => include_bytes!("../../../assets/file-types/png/light/16/folder.png"),
            Self::GenericFile => {
                include_bytes!("../../../assets/file-types/png/light/16/file.png")
            }
            Self::Pdf => include_bytes!("../../../assets/file-types/png/light/16/pdf.png"),
            Self::Text => include_bytes!("../../../assets/file-types/png/light/16/text.png"),
            Self::Markdown => {
                include_bytes!("../../../assets/file-types/png/light/16/markdown.png")
            }
            Self::Rust => include_bytes!("../../../assets/file-types/png/light/16/rust.png"),
            Self::Shell => include_bytes!("../../../assets/file-types/png/light/16/shell.png"),
            Self::Config => include_bytes!("../../../assets/file-types/png/light/16/config.png"),
            Self::Json => include_bytes!("../../../assets/file-types/png/light/16/json.png"),
            Self::Image => include_bytes!("../../../assets/file-types/png/light/16/image.png"),
            Self::Archive => include_bytes!("../../../assets/file-types/png/light/16/archive.png"),
            Self::Database => {
                include_bytes!("../../../assets/file-types/png/light/16/database.png")
            }
            Self::Key => include_bytes!("../../../assets/file-types/png/light/16/key.png"),
            Self::Certificate => {
                include_bytes!("../../../assets/file-types/png/light/16/certificate.png")
            }
            Self::Executable => {
                include_bytes!("../../../assets/file-types/png/light/16/executable.png")
            }
            Self::Code => include_bytes!("../../../assets/file-types/png/light/16/rust.png"),
            Self::Log => include_bytes!("../../../assets/file-types/png/light/16/log.png"),
            Self::Symlink => include_bytes!("../../../assets/file-types/png/light/16/symlink.png"),
        }
    }
}

fn decode_image(bytes: &[u8]) -> ColorImage {
    let img = image::load_from_memory(bytes)
        .expect("Valid PNG asset")
        .to_rgba8();
    let (width, height) = img.dimensions();
    ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &img.into_raw())
}

pub fn ui_icon_texture(ctx: &Context, icon: UiIcon, dark: bool) -> TextureHandle {
    let name = format!("ui_icon_{:?}_{}", icon, if dark { "dark" } else { "light" });
    ctx.load_texture(
        name,
        decode_image(if dark {
            icon.dark_png_bytes()
        } else {
            icon.light_png_bytes()
        }),
        TextureOptions::LINEAR,
    )
}

pub fn file_icon_texture(ctx: &Context, file_type: FileType, dark: bool) -> TextureHandle {
    let name = format!(
        "file_icon_{:?}_{}",
        file_type,
        if dark { "dark" } else { "light" }
    );
    ctx.load_texture(
        name,
        decode_image(if dark {
            file_type.dark_png_bytes()
        } else {
            file_type.light_png_bytes()
        }),
        TextureOptions::LINEAR,
    )
}

pub fn monogram_texture(ctx: &Context, dark: bool) -> TextureHandle {
    let name = format!("kervesh_monogram_{}", if dark { "white" } else { "black" });
    let bytes: &'static [u8] = if dark {
        include_bytes!("../../../assets/brand/png/kervesh-mark-white-256.png")
    } else {
        include_bytes!("../../../assets/brand/png/kervesh-mark-black-256.png")
    };
    ctx.load_texture(name, decode_image(bytes), TextureOptions::LINEAR)
}

pub fn render_monogram(ui: &mut Ui, size: f32, color: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if ui.is_rect_visible(rect) {
        let dark = color.r() > 100;
        let texture = monogram_texture(ui.ctx(), dark);
        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter().image(texture.id(), rect, uv, Color32::WHITE);
    }
    response
}

pub fn paint_monogram(painter: &Painter, rect: Rect, color: Color32) {
    let dark = color.r() > 100;
    let texture = monogram_texture(painter.ctx(), dark);
    let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    painter.image(texture.id(), rect, uv, Color32::WHITE);
}

pub fn render_file_icon(ui: &mut Ui, file_type: FileType, color: Color32) -> Response {
    let size = Vec2::splat(16.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_file_icon(ui.painter(), rect, file_type, color);
    }
    response
}

pub fn paint_file_icon(painter: &Painter, rect: Rect, file_type: FileType, color: Color32) {
    let dark = color.r() > 100;
    let texture = file_icon_texture(painter.ctx(), file_type, dark);
    let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    painter.image(texture.id(), rect, uv, Color32::WHITE);
}

pub fn render_ui_icon(ui: &mut Ui, icon: UiIcon, size: f32, dark: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if ui.is_rect_visible(rect) {
        let texture = ui_icon_texture(ui.ctx(), icon, dark);
        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter().image(texture.id(), rect, uv, Color32::WHITE);
    }
    response
}

pub fn ui_icon_button(ui: &mut Ui, icon: UiIcon, tooltip: &str, dark: bool) -> Response {
    let size = Vec2::splat(22.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        if response.hovered() {
            let bg = if dark {
                crate::theme::colors::SLATE
            } else {
                crate::theme::colors::LIGHT_BORDER
            };
            ui.painter().rect_filled(rect, 4.0_f32, bg);
        }
        let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(16.0));
        let texture = ui_icon_texture(ui.ctx(), icon, dark);
        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter()
            .image(texture.id(), icon_rect, uv, Color32::WHITE);
    }
    response.on_hover_text(tooltip)
}

pub fn ui_icon_label_button(ui: &mut Ui, icon: UiIcon, label: &str, dark: bool) -> Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui.fonts_mut(|f| {
        f.layout_no_wrap(
            label.into(),
            font,
            if dark {
                crate::theme::colors::FOREGROUND
            } else {
                crate::theme::colors::LIGHT_FOREGROUND
            },
        )
    });
    let text_w = galley.rect.width();
    let total_w = 20.0 + text_w + 14.0;
    let total_h = 26.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(total_w, total_h), Sense::click());
    if ui.is_rect_visible(rect) {
        let bg = if response.is_pointer_button_down_on() {
            if dark {
                crate::theme::colors::SLATE
            } else {
                crate::theme::colors::LIGHT_BORDER
            }
        } else if response.hovered() {
            if dark {
                crate::theme::colors::GRAPHITE
            } else {
                crate::theme::colors::LIGHT_PANEL_RAISED
            }
        } else {
            if dark {
                crate::theme::colors::DARK_PANEL_RAISED
            } else {
                crate::theme::colors::LIGHT_PANEL
            }
        };
        let border = if dark {
            crate::theme::colors::DARK_BORDER
        } else {
            crate::theme::colors::LIGHT_BORDER
        };
        ui.painter().rect(
            rect,
            4.0_f32,
            bg,
            egui::Stroke::new(1.0_f32, border),
            egui::StrokeKind::Inside,
        );

        let icon_rect = Rect::from_min_size(
            egui::pos2(rect.min.x + 6.0, rect.center().y - 8.0),
            Vec2::splat(16.0),
        );
        let texture = ui_icon_texture(ui.ctx(), icon, dark);
        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter()
            .image(texture.id(), icon_rect, uv, Color32::WHITE);

        let text_pos = egui::pos2(
            rect.min.x + 26.0,
            rect.center().y - galley.rect.height() * 0.5,
        );
        ui.painter().galley(
            text_pos,
            galley,
            if dark {
                crate::theme::colors::FOREGROUND
            } else {
                crate::theme::colors::LIGHT_FOREGROUND
            },
        );
    }
    response
}

pub fn paint_sparkline(painter: &Painter, rect: Rect, values: &[f32], color: Color32) {
    if values.len() < 2 {
        let y_mid = rect.center().y;
        painter.line_segment(
            [egui::pos2(rect.min.x, y_mid), egui::pos2(rect.max.x, y_mid)],
            egui::Stroke::new(1.0_f32, color.gamma_multiply(0.3)),
        );
        return;
    }
    let max_val = values.iter().copied().fold(100.0_f32, f32::max).max(1.0);
    let step_x = rect.width() / (values.len() - 1) as f32;
    let points: Vec<egui::Pos2> = values
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let x = rect.min.x + i as f32 * step_x;
            let ratio = (v / max_val).clamp(0.0, 1.0);
            let y = rect.max.y - ratio * (rect.height() - 4.0) - 2.0;
            egui::pos2(x, y)
        })
        .collect();
    painter.add(egui::epaint::PathShape::line(
        points,
        egui::Stroke::new(1.5_f32, color),
    ));
}
