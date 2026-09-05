use crate::Terminal;
use alacritty_terminal::{
    index::{Column, Point},
    term::viewport_to_point,
    vte,
};

pub fn safe_url(value: &str) -> bool {
    (value.starts_with("https://") || value.starts_with("http://"))
        && value
            .split_once("://")
            .is_some_and(|(_, rest)| !rest.is_empty() && !rest.starts_with('/'))
        && value.len() <= 4096
        && !value.chars().any(|c| c.is_control() || c.is_whitespace())
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteDirectory {
    pub host: String,
    pub path: String,
}
#[derive(Default)]
pub(crate) struct Metadata {
    pub directory: Option<RemoteDirectory>,
}
impl vte::Perform for Metadata {
    fn osc_dispatch(&mut self, params: &[&[u8]], _: bool) {
        if params.first() != Some(&b"7".as_slice()) {
            return;
        }
        self.directory = None;
        if params.len() != 2 {
            return;
        }
        let Ok(value) = std::str::from_utf8(params[1]) else {
            return;
        };
        let Some(rest) = value.strip_prefix("file://") else {
            return;
        };
        let Some((host, path)) = rest.split_once('/') else {
            return;
        };
        if host.len() > 255
            || host
                .chars()
                .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '@' | '?' | '#'))
        {
            return;
        }
        let mut bytes = Vec::new();
        let mut iter = path.bytes();
        while let Some(b) = iter.next() {
            if b == b'%' {
                let Some(a) = iter.next().and_then(|c| (c as char).to_digit(16)) else {
                    return;
                };
                let Some(b) = iter.next().and_then(|c| (c as char).to_digit(16)) else {
                    return;
                };
                bytes.push((a * 16 + b) as u8);
            } else {
                bytes.push(b);
            }
        }
        let Ok(path) = String::from_utf8(bytes) else {
            return;
        };
        if path.len() > 4095 || path.chars().any(char::is_control) {
            return;
        }
        self.directory = Some(RemoteDirectory {
            host: host.into(),
            path: format!("/{path}"),
        });
    }
}
impl Terminal {
    pub fn directory(&self) -> Option<&RemoteDirectory> {
        self.metadata.directory.as_ref()
    }
    pub fn hyperlink_at(&self, row: usize, col: usize) -> Option<String> {
        if row >= self.rows || col >= self.cols {
            return None;
        }
        let point = viewport_to_point(
            self.term.grid().display_offset(),
            Point::new(row, Column(col)),
        );
        if let Some(link) = self.term.grid()[point].hyperlink() {
            return safe_url(link.uri()).then(|| link.uri().to_owned());
        }
        let token = self.token_at(row, col)?;
        let url = token.trim_end_matches(['.', ',', ';', ')', ']', '}']);
        safe_url(url).then(|| url.to_owned())
    }
    pub(crate) fn token_at(&self, row: usize, col: usize) -> Option<String> {
        if row >= self.rows || col >= self.cols {
            return None;
        }
        let point = viewport_to_point(
            self.term.grid().display_offset(),
            Point::new(row, Column(col)),
        );
        let line = &self.term.grid()[point.line];
        let delimiter = |c: char| c.is_whitespace() || matches!(c, '\'' | '"' | '<' | '>');
        if delimiter(line[Column(col)].c) {
            return None;
        }
        let mut start = col;
        let mut end = col + 1;
        while start > 0 && !delimiter(line[Column(start - 1)].c) {
            start -= 1;
        }
        while end < self.cols && !delimiter(line[Column(end)].c) {
            end += 1;
        }
        Some((start..end).map(|c| line[Column(c)].c).collect())
    }
}

/// Bounded OSC observer; terminal state remains exclusively owned by Alacritty.
/// Ignore control strings so embedded OSC-looking bytes cannot become metadata.
#[derive(Default)]
pub(crate) struct MetadataParser {
    state: u8,
    bytes: Vec<u8>,
    overflow: bool,
}
impl MetadataParser {
    pub fn advance(&mut self, metadata: &mut Metadata, bytes: &[u8]) {
        for &byte in bytes {
            match self.state {
                0 => {
                    if byte == 27 {
                        self.state = 1;
                    }
                }
                1 => match byte {
                    b']' => {
                        self.state = 2;
                        self.bytes.clear();
                        self.overflow = false;
                    }
                    b'P' | b'X' | b'^' | b'_' => self.state = 4,
                    27 => {}
                    _ => self.state = 0,
                },
                2 => match byte {
                    7 => self.finish(metadata),
                    27 => self.state = 3,
                    24 | 26 => {
                        self.bytes.clear();
                        self.state = 0;
                    }
                    _ if self.bytes.len() < 8192 && !self.overflow => self.bytes.push(byte),
                    _ => {
                        self.overflow = true;
                        self.bytes.clear();
                    }
                },
                3 => {
                    if byte == b'\\' {
                        self.finish(metadata);
                    } else {
                        self.bytes.clear();
                        self.state = 1;
                        self.advance(metadata, &[byte]);
                    }
                }
                4 => match byte {
                    27 => self.state = 5,
                    24 | 26 => self.state = 0,
                    _ => {}
                },
                5 => match byte {
                    b'\\' | 24 | 26 => self.state = 0,
                    27 => {}
                    _ => self.state = 4,
                },
                _ => unreachable!(),
            }
        }
    }
    fn finish(&mut self, metadata: &mut Metadata) {
        if !self.overflow {
            let params: Vec<_> = self.bytes.split(|b| *b == b';').take(3).collect();
            vte::Perform::osc_dispatch(metadata, &params, false);
        }
        self.bytes.clear();
        self.state = 0;
    }
}
