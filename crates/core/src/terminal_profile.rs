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
