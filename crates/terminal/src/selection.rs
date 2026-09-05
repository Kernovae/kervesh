use crate::Terminal;
use alacritty_terminal::{
    index::{Column, Point, Side},
    selection::{Selection, SelectionType},
    term::viewport_to_point,
};

impl Terminal {
    pub fn selection_text(&self) -> Option<String> {
        self.term.selection_to_string().filter(|s| !s.is_empty())
    }
    pub fn clear_selection(&mut self) {
        self.term.selection = None;
    }
    pub fn select_range(&mut self, start: (usize, usize), end: (usize, usize)) {
        let offset = self.term.grid().display_offset();
        let mut selection = Selection::new(
            SelectionType::Simple,
            viewport_to_point(offset, Point::new(start.0, Column(start.1))),
            Side::Left,
        );
        selection.update(
            viewport_to_point(offset, Point::new(end.0, Column(end.1))),
            Side::Right,
        );
        self.term.selection = Some(selection);
    }
    pub fn select_word(&mut self, row: usize, col: usize) {
        self.select_kind(row, col, SelectionType::Semantic);
    }
    pub fn select_line(&mut self, row: usize) {
        self.select_kind(row, 0, SelectionType::Lines);
    }
    fn select_kind(&mut self, row: usize, col: usize, kind: SelectionType) {
        let point = viewport_to_point(
            self.term.grid().display_offset(),
            Point::new(row, Column(col)),
        );
        self.term.selection = Some(Selection::new(kind, point, Side::Left));
    }
}
