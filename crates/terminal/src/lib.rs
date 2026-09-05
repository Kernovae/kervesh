mod glyph;
mod hyperlink;
mod input;
mod renderer;
mod search;
mod selection;
mod widget;
use alacritty_terminal::{
    Term,
    event::{Event, EventListener},
    grid::Dimensions,
    term::{Config, Osc52},
    vte::ansi,
};
pub use glyph::*;
pub use hyperlink::{RemoteDirectory, safe_url};
pub use input::*;
pub use search::SearchMatch;
use std::sync::{Arc, Mutex};
mod font;
pub use font::*;
pub use kervesh_core::{
    ClipboardProfile, MultilinePastePolicy, TerminalCursor, TerminalPalette, TerminalProfile,
};

struct Size(usize, usize);
impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.1
    }
    fn screen_lines(&self) -> usize {
        self.1
    }
    fn columns(&self) -> usize {
        self.0
    }
}
#[derive(Clone, Default)]
struct Listener(Arc<Mutex<Vec<u8>>>, Arc<std::sync::atomic::AtomicBool>);
impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        if matches!(event, Event::Bell) {
            self.1.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        if let Event::PtyWrite(text) = event
            && let Ok(mut bytes) = self.0.lock()
        {
            bytes.extend_from_slice(text.as_bytes());
        }
    }
}
pub struct Terminal {
    term: Term<Listener>,
    parser: ansi::Processor,
    listener: Listener,
    cols: usize,
    rows: usize,
    profile: TerminalProfile,
    font_config: TerminalFontConfig,
    search: search::SearchState,
    metadata: hyperlink::Metadata,
    metadata_parser: hyperlink::MetadataParser,
    pending_paste: Option<String>,
    paste_requested: bool,
    restore_focus: bool,
    bell_until: f64,
    audio_after: f64,
}
#[derive(Default)]
pub struct TerminalAction {
    pub input: Vec<u8>,
    pub resize: Option<(u32, u32)>,
    pub reveal_path: Option<String>,
    pub audio_bell: bool,
}
impl Terminal {
    pub fn new(cols: usize, rows: usize, scrollback: usize) -> Self {
        let listener = Listener::default();
        let cols = cols.max(2);
        let rows = rows.max(1);
        let config = Config {
            scrolling_history: scrollback.min(100000),
            osc52: Osc52::Disabled,
            ..Config::default()
        };
        Self {
            term: Term::new(config, &Size(cols, rows), listener.clone()),
            parser: ansi::Processor::new(),
            listener,
            cols,
            rows,
            profile: TerminalProfile {
                scrollback: scrollback.min(100000),
                ..TerminalProfile::default()
            },
            font_config: TerminalFontConfig::from(&TerminalProfile::default()),
            search: Default::default(),
            metadata: Default::default(),
            metadata_parser: Default::default(),
            pending_paste: None,
            paste_requested: false,
            restore_focus: false,
            bell_until: 0.0,
            audio_after: 0.0,
        }
    }
    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
        self.metadata_parser.advance(&mut self.metadata, bytes);
        self.search.dirty = true;
    }
    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.cols = cols.max(2);
        self.rows = rows.max(1);
        self.term.resize(Size(self.cols, self.rows));
        self.search.dirty = true;
    }
    pub fn text(&self) -> String {
        use alacritty_terminal::index::{Column, Line};
        (0..self.rows)
            .map(|row| {
                (0..self.cols)
                    .map(|col| self.term.grid()[Line(row as i32)][Column(col)].c)
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn replies(&self) -> Vec<u8> {
        self.listener
            .0
            .lock()
            .map(|mut b| std::mem::take(&mut *b))
            .unwrap_or_default()
    }
}

mod clipboard;
pub use clipboard::*;

impl Terminal {
    pub fn with_profile(cols: usize, rows: usize, profile: TerminalProfile) -> Self {
        let mut terminal = Self::new(cols, rows, profile.scrollback);
        terminal.set_profile(profile);
        terminal
    }
    pub fn profile(&self) -> &TerminalProfile {
        &self.profile
    }
    pub fn set_profile(&mut self, profile: TerminalProfile) {
        if profile.validate().is_err() || self.profile == profile {
            return;
        }
        let shape = match profile.cursor_style {
            TerminalCursor::Block => ansi::CursorShape::Block,
            TerminalCursor::Beam => ansi::CursorShape::Beam,
            TerminalCursor::Underline => ansi::CursorShape::Underline,
        };
        self.term.set_options(Config {
            scrolling_history: profile.scrollback,
            osc52: Osc52::Disabled,
            default_cursor_style: ansi::CursorStyle {
                shape,
                blinking: profile.cursor_blink,
            },
            ..Config::default()
        });
        self.font_config = TerminalFontConfig::from(&profile);
        self.profile = profile;
        self.search.dirty = true;
    }
}
