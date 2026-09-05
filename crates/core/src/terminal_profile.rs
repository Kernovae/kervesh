use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipboardProfile {
    #[default]
    Desktop,
    Traditional,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MultilinePastePolicy {
    Off,
    #[default]
    Warn,
    AlwaysPreview,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalCursor {
    #[default]
    Block,
    Beam,
    Underline,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaletteKind {
    #[default]
    KerveshDark,
    KerveshLight,
    Custom,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TerminalPalette {
    pub kind: PaletteKind,
    pub background: [u8; 3],
    pub foreground: [u8; 3],
    pub cursor: [u8; 3],
    pub selection: [u8; 3],
    pub ansi: [[u8; 3]; 16],
}
impl Default for TerminalPalette {
    fn default() -> Self {
        Self {
            kind: PaletteKind::KerveshDark,
            background: [17, 21, 25],
            foreground: [215, 223, 225],
            cursor: [100, 202, 160],
            selection: [44, 76, 96],
            ansi: [
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
            ],
        }
    }
}
impl TerminalPalette {
    pub fn light() -> Self {
        Self {
            kind: PaletteKind::KerveshLight,
            background: [250, 250, 247],
            foreground: [30, 38, 42],
            cursor: [20, 100, 70],
            selection: [170, 200, 220],
            ansi: [
                [30, 38, 42],
                [170, 35, 40],
                [25, 115, 55],
                [130, 95, 10],
                [35, 80, 165],
                [120, 50, 145],
                [15, 105, 115],
                [195, 200, 200],
                [85, 95, 105],
                [200, 55, 55],
                [40, 140, 70],
                [155, 115, 20],
                [55, 105, 190],
                [145, 75, 170],
                [25, 130, 140],
                [240, 243, 245],
            ],
        }
    }

    pub fn dracula() -> Self {
        Self {
            kind: PaletteKind::Custom,
            background: [40, 42, 54],
            foreground: [248, 248, 242],
            cursor: [248, 248, 242],
            selection: [68, 71, 90],
            ansi: [
                [33, 34, 44],
                [255, 85, 85],
                [80, 250, 123],
                [241, 250, 140],
                [189, 147, 249],
                [255, 121, 198],
                [139, 233, 253],
                [191, 191, 191],
                [98, 114, 164],
                [255, 110, 110],
                [105, 250, 142],
                [244, 250, 160],
                [202, 169, 250],
                [255, 146, 209],
                [164, 238, 253],
                [255, 255, 255],
            ],
        }
    }

    pub fn nord() -> Self {
        Self {
            kind: PaletteKind::Custom,
            background: [46, 52, 64],
            foreground: [236, 239, 244],
            cursor: [236, 239, 244],
            selection: [76, 86, 106],
            ansi: [
                [59, 66, 82],
                [191, 97, 106],
                [163, 190, 140],
                [235, 203, 139],
                [129, 161, 193],
                [180, 142, 173],
                [136, 192, 208],
                [229, 233, 240],
                [76, 86, 106],
                [208, 135, 141],
                [180, 203, 161],
                [239, 214, 160],
                [143, 188, 187],
                [194, 163, 188],
                [143, 188, 187],
                [236, 239, 244],
            ],
        }
    }

    pub fn tokyo_night() -> Self {
        Self {
            kind: PaletteKind::Custom,
            background: [26, 27, 38],
            foreground: [192, 202, 245],
            cursor: [192, 202, 245],
            selection: [51, 70, 110],
            ansi: [
                [21, 22, 30],
                [247, 118, 142],
                [158, 206, 106],
                [224, 175, 104],
                [122, 162, 247],
                [187, 154, 247],
                [125, 207, 255],
                [169, 177, 214],
                [65, 72, 104],
                [247, 118, 142],
                [158, 206, 106],
                [224, 175, 104],
                [122, 162, 247],
                [187, 154, 247],
                [125, 207, 255],
                [192, 202, 245],
            ],
        }
    }

    pub fn catppuccin_mocha() -> Self {
        Self {
            kind: PaletteKind::Custom,
            background: [30, 30, 46],
            foreground: [205, 214, 244],
            cursor: [245, 224, 220],
            selection: [88, 91, 112],
            ansi: [
                [69, 71, 90],
                [243, 139, 168],
                [166, 227, 161],
                [249, 226, 175],
                [137, 180, 250],
                [245, 194, 231],
                [148, 226, 213],
                [186, 194, 222],
                [88, 91, 112],
                [243, 139, 168],
                [166, 227, 161],
                [249, 226, 175],
                [137, 180, 250],
                [245, 194, 231],
                [148, 226, 213],
                [166, 173, 200],
            ],
        }
    }

    pub fn gruvbox_dark() -> Self {
        Self {
            kind: PaletteKind::Custom,
            background: [40, 40, 40],
            foreground: [235, 219, 178],
            cursor: [235, 219, 178],
            selection: [80, 73, 69],
            ansi: [
                [40, 40, 40],
                [204, 36, 29],
                [152, 151, 26],
                [215, 153, 33],
                [69, 133, 136],
                [177, 98, 134],
                [104, 157, 106],
                [168, 153, 132],
                [146, 131, 116],
                [251, 73, 52],
                [184, 187, 38],
                [250, 189, 47],
                [131, 165, 152],
                [211, 134, 155],
                [142, 192, 124],
                [235, 219, 178],
            ],
        }
    }

    pub fn one_dark() -> Self {
        Self {
            kind: PaletteKind::Custom,
            background: [40, 44, 52],
            foreground: [171, 178, 191],
            cursor: [82, 139, 255],
            selection: [62, 68, 81],
            ansi: [
                [40, 44, 52],
                [224, 108, 117],
                [152, 195, 121],
                [229, 192, 123],
                [97, 175, 239],
                [198, 120, 221],
                [86, 182, 194],
                [171, 178, 191],
                [92, 99, 112],
                [224, 108, 117],
                [152, 195, 121],
                [229, 192, 123],
                [97, 175, 239],
                [198, 120, 221],
                [86, 182, 194],
                [255, 255, 255],
            ],
        }
    }

    /// Calculates sRGB relative luminance according to WCAG specifications
    pub fn relative_luminance(rgb: [u8; 3]) -> f32 {
        let channel = |c: u8| -> f32 {
            let s = c as f32 / 255.0;
            if s <= 0.03928 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2])
    }

    /// Calculates WCAG contrast ratio between two colors (ranging from 1.0 to 21.0)
    pub fn contrast_ratio(c1: [u8; 3], c2: [u8; 3]) -> f32 {
        let l1 = Self::relative_luminance(c1);
        let l2 = Self::relative_luminance(c2);
        let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
        (lighter + 0.05) / (darker + 0.05)
    }

    /// Checks if foreground/background meet WCAG AA standard (>= 4.5:1)
    pub fn is_wcag_aa(c1: [u8; 3], c2: [u8; 3]) -> bool {
        Self::contrast_ratio(c1, c2) >= 4.5
    }

    pub fn export_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn import_json(json: &str) -> Result<Self> {
        let palette: Self = serde_json::from_str(json)?;
        Ok(palette)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TerminalProfile {
    pub id: String,
    pub name: String,
    /// Built-in Hack, or an explicitly selected local TTF/OTF path.
    pub font_family: String,
    pub font_fallbacks: Vec<String>,
    pub font_size: f32,
    pub line_height: f32,
    pub cursor_style: TerminalCursor,
    pub cursor_blink: bool,
    pub scrollback: usize,
    pub clipboard_profile: ClipboardProfile,
    pub copy_on_select: bool,
    pub multiline_paste_policy: MultilinePastePolicy,
    pub literal_control_keys: bool,
    pub palette: TerminalPalette,
    pub bell_visual: bool,
    pub bell_audio: bool,
    pub padding: f32,
    pub hyperlinks_enabled: bool,
    pub follow_terminal_directory: bool,
}
impl Default for TerminalProfile {
    fn default() -> Self {
        Self {
            id: "default".into(),
            name: "Default".into(),
            font_family: "Hack".into(),
            font_fallbacks: vec![],
            font_size: 14.0,
            line_height: 1.0,
            cursor_style: TerminalCursor::Block,
            cursor_blink: false,
            scrollback: 10000,
            clipboard_profile: ClipboardProfile::Desktop,
            copy_on_select: false,
            multiline_paste_policy: MultilinePastePolicy::Warn,
            literal_control_keys: true,
            palette: TerminalPalette::default(),
            bell_visual: true,
            bell_audio: false,
            padding: 4.0,
            hyperlinks_enabled: true,
            follow_terminal_directory: false,
        }
    }
}
impl TerminalProfile {
    pub fn builtins() -> Vec<Self> {
        let mut profiles = vec![Self::default()];
        for (id, name) in [
            ("server-administration", "Server Administration"),
            ("development", "Development"),
            ("database", "Database"),
            ("minimal", "Minimal"),
        ] {
            let mut p = Self {
                id: id.into(),
                name: name.into(),
                ..Self::default()
            };
            match id {
                "server-administration" => {
                    p.multiline_paste_policy = MultilinePastePolicy::AlwaysPreview
                }
                "development" => {
                    p.scrollback = 50000;
                    p.cursor_style = TerminalCursor::Beam;
                }
                "database" => p.scrollback = 30000,
                "minimal" => {
                    p.scrollback = 1000;
                    p.padding = 0.0;
                    p.bell_visual = false;
                }
                _ => {}
            }
            profiles.push(p);
        }
        profiles
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.id.is_empty()
                && self.id.len() <= 128
                && self
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == 45 || b == 95),
            "Invalid terminal profile ID"
        );
        ensure!(
            !self.name.trim().is_empty()
                && self.name.len() <= 128
                && !self.name.chars().any(char::is_control),
            "Invalid terminal profile name"
        );
        ensure!(
            self.font_size.is_finite() && (8.0..=32.0).contains(&self.font_size),
            "Terminal font size must be 8–32"
        );
        ensure!(
            self.line_height.is_finite() && (1.0..=2.0).contains(&self.line_height),
            "Line height must be 1–2"
        );
        ensure!(
            self.padding.is_finite() && (0.0..=32.0).contains(&self.padding),
            "Padding must be 0–32"
        );
        ensure!(
            self.scrollback <= 100000,
            "Scrollback limit is 100000 lines"
        );
        ensure!(
            self.font_fallbacks.len() <= 8,
            "At most eight font fallbacks"
        );
        for font in std::iter::once(&self.font_family).chain(&self.font_fallbacks) {
            ensure!(
                !font.is_empty() && font.len() <= 4096 && !font.chars().any(char::is_control),
                "Invalid terminal font path"
            );
        }
        Ok(())
    }
}
