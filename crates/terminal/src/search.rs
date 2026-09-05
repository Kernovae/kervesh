use crate::Terminal;
use alacritty_terminal::{
    grid::Dimensions,
    index::{Column, Line, Point},
    term::cell::Flags,
};

#[derive(Clone, Debug)]
pub struct SearchMatch {
    pub start: Point,
    pub end: Point,
}
#[derive(Default)]
pub(crate) struct SearchState {
    pub open: bool,
    pub focus: bool,
    pub query: String,
    pub case_sensitive: bool,
    pub matches: Vec<SearchMatch>,
    pub current: usize,
    pub dirty: bool,
}
impl Terminal {
    pub fn search(&mut self, query: &str, case_sensitive: bool) {
        self.search.query = query.into();
        self.search.case_sensitive = case_sensitive;
        self.refresh_search();
    }
    pub fn search_matches(&self) -> &[SearchMatch] {
        &self.search.matches
    }
    pub(crate) fn refresh_search(&mut self) {
        self.search.matches.clear();
        self.search.current = 0;
        self.search.dirty = false;
        if self.search.query.is_empty() {
            return;
        }
        let needle = if self.search.case_sensitive {
            self.search.query.clone()
        } else {
            self.search.query.to_lowercase()
        };
        let mut text = String::new();
        let mut points = Vec::new();
        for row in -(self.term.grid().history_size() as i32)..self.rows as i32 {
            for col in 0..self.cols {
                let point = Point::new(Line(row), Column(col));
                let cell = &self.term.grid()[point];
                if cell
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    continue;
                }
                for c in std::iter::once(cell.c)
                    .chain(cell.zerowidth().unwrap_or_default().iter().copied())
                {
                    if self.search.case_sensitive {
                        text.push(c);
                        points.extend(std::iter::repeat_n(point, c.len_utf8()));
                    } else {
                        for c in c.to_lowercase() {
                            text.push(c);
                            points.extend(std::iter::repeat_n(point, c.len_utf8()));
                        }
                    }
                }
            }
            if !self.term.grid()[Line(row)][Column(self.cols - 1)]
                .flags
                .contains(Flags::WRAPLINE)
                || row == self.rows as i32 - 1
            {
                for (index, _) in text.match_indices(&needle) {
                    self.search.matches.push(SearchMatch {
                        start: points[index],
                        end: points[index + needle.len() - 1],
                    });
                    // Bound highlights for adversarial output and very broad queries.
                    if self.search.matches.len() == 10000 {
                        return;
                    }
                }
                text.clear();
                points.clear();
            }
        }
    }
    pub(crate) fn navigate_match(&mut self, next: bool) {
        let len = self.search.matches.len();
        if len == 0 {
            return;
        }
        self.search.current = if next {
            (self.search.current + 1) % len
        } else {
            (self.search.current + len - 1) % len
        };
        self.term
            .scroll_to_point(self.search.matches[self.search.current].start);
    }
}
