mod input;
mod widget;
use alacritty_terminal::{
    Term,
    event::{Event, EventListener},
    grid::Dimensions,
    term::{Config, Osc52},
    vte::ansi,
};
pub use input::*;
use std::sync::{Arc, Mutex};
pub use widget::{draw_custom_glyph, is_custom_glyph};

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
struct Listener(Arc<Mutex<Vec<u8>>>);
impl EventListener for Listener {
    fn send_event(&self, event: Event) {
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
}
#[derive(Default)]
pub struct TerminalAction {
    pub input: Vec<u8>,
    pub resize: Option<(u32, u32)>,
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
        }
    }
    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }
    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.cols = cols.max(2);
        self.rows = rows.max(1);
        self.term.resize(Size(self.cols, self.rows));
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
