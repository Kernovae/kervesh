#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    Crlf,
}

impl LineEnding {
    pub fn as_str(&self) -> &'static str {
        match self {
            LineEnding::Lf => "LF",
            LineEnding::Crlf => "CRLF",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntaxLanguage {
    Plain,
    Rust,
    Python,
    Bash,
    Json,
    Yaml,
    Toml,
    Sql,
    Markdown,
    Docker,
    Config,
}

impl SyntaxLanguage {
    pub fn from_path(path: &str) -> Self {
        let lower = path.to_lowercase();
        if lower.ends_with(".rs") {
            Self::Rust
        } else if lower.ends_with(".py") {
            Self::Python
        } else if lower.ends_with(".sh") || lower.ends_with(".bash") || lower.ends_with(".zsh") {
            Self::Bash
        } else if lower.ends_with(".json") {
            Self::Json
        } else if lower.ends_with(".yaml") || lower.ends_with(".yml") {
            Self::Yaml
        } else if lower.ends_with(".toml") {
            Self::Toml
        } else if lower.ends_with(".sql") {
            Self::Sql
        } else if lower.ends_with(".md") || lower.ends_with(".markdown") {
            Self::Markdown
        } else if lower.ends_with("dockerfile") || lower.contains("dockerfile") {
            Self::Docker
        } else if lower.ends_with(".conf") || lower.ends_with(".cfg") || lower.ends_with(".ini") {
            Self::Config
        } else {
            Self::Plain
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Plain => "Plain Text",
            Self::Rust => "Rust",
            Self::Python => "Python",
            Self::Bash => "Shell",
            Self::Json => "JSON",
            Self::Yaml => "YAML",
            Self::Toml => "TOML",
            Self::Sql => "SQL",
            Self::Markdown => "Markdown",
            Self::Docker => "Dockerfile",
            Self::Config => "Config",
        }
    }
}

pub struct RemoteEditor {
    pub name: String,
    pub path: String,
    pub content: String,
    pub original_content: String,
    pub dirty: bool,
    pub saving: bool,
    pub error: Option<String>,
    pub line_ending: LineEnding,
    pub syntax: SyntaxLanguage,
    pub search_open: bool,
    pub search_query: String,
    pub replace_query: String,
    pub case_sensitive: bool,
    pub go_to_line_open: bool,
    pub go_to_line_input: String,
    pub save_as_open: bool,
    pub save_as_input: String,
    pub conflict_warning: Option<String>,
}

impl RemoteEditor {
    pub fn new(path: String, content: String) -> Self {
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        let line_ending = if content.contains("\r\n") {
            LineEnding::Crlf
        } else {
            LineEnding::Lf
        };
        let syntax = SyntaxLanguage::from_path(&path);
        Self {
            name,
            save_as_input: path.clone(),
            path,
            line_ending,
            syntax,
            original_content: content.clone(),
            content,
            dirty: false,
            saving: false,
            error: None,
            search_open: false,
            search_query: String::new(),
            replace_query: String::new(),
            case_sensitive: false,
            go_to_line_open: false,
            go_to_line_input: String::new(),
            save_as_open: false,
            conflict_warning: None,
        }
    }

    pub fn line_count(&self) -> usize {
        self.content.lines().count().max(1)
    }

    pub fn prepare_save_content(&self) -> String {
        match self.line_ending {
            LineEnding::Lf => self.content.replace("\r\n", "\n"),
            LineEnding::Crlf => {
                let normalized = self.content.replace("\r\n", "\n");
                normalized.replace('\n', "\r\n")
            }
        }
    }

    pub fn count_search_matches(&self) -> usize {
        if self.search_query.is_empty() {
            return 0;
        }
        if self.case_sensitive {
            self.content.matches(&self.search_query).count()
        } else {
            case_insensitive_ranges(&self.content, &self.search_query).len()
        }
    }

    pub fn replace_next(&mut self) -> bool {
        if self.search_query.is_empty() {
            return false;
        }
        if let Some((start, end)) = if self.case_sensitive {
            self.content
                .find(&self.search_query)
                .map(|start| (start, start + self.search_query.len()))
        } else {
            case_insensitive_ranges(&self.content, &self.search_query)
                .into_iter()
                .next()
        } {
            self.content.replace_range(start..end, &self.replace_query);
            self.dirty = true;
            true
        } else {
            false
        }
    }

    pub fn replace_all(&mut self) -> usize {
        if self.search_query.is_empty() {
            return 0;
        }
        let count = self.count_search_matches();
        if count > 0 {
            if self.case_sensitive {
                self.content = self
                    .content
                    .replace(&self.search_query, &self.replace_query);
            } else {
                let ranges = case_insensitive_ranges(&self.content, &self.search_query);
                let mut result = self.content.clone();
                for (start, end) in ranges.into_iter().rev() {
                    result.replace_range(start..end, &self.replace_query);
                }
                self.content = result;
            }
            self.dirty = true;
        }
        count
    }
}

/// Return byte ranges in the original string for matches in its Unicode lowercase form.
/// Lowercase conversion can expand a character (for example, `İ` becomes `i` plus a
/// combining dot), so offsets from the transformed string must never be used directly
/// with the source string.
fn case_insensitive_ranges(content: &str, query: &str) -> Vec<(usize, usize)> {
    let lower_query = fold_case(query);
    if lower_query.is_empty() {
        return Vec::new();
    }

    let mut lower_content = String::with_capacity(content.len());
    let mut lower_char_ranges = Vec::new();
    for (start, ch) in content.char_indices() {
        let end = start + ch.len_utf8();
        for lower_ch in fold_case(&ch.to_string()).chars() {
            lower_content.push(lower_ch);
            lower_char_ranges.push((start, end));
        }
    }

    let mut ranges = lower_content
        .match_indices(&lower_query)
        .filter_map(|(lower_start, matched)| {
            let lower_end = lower_start + matched.len();
            let start_char = lower_content[..lower_start].chars().count();
            let end_char = lower_content[..lower_end].chars().count();
            let start = lower_char_ranges.get(start_char)?.0;
            let end = lower_char_ranges.get(end_char.checked_sub(1)?)?.1;
            Some((start, end))
        })
        .collect::<Vec<_>>();
    ranges.sort_unstable_by_key(|(start, _)| *start);
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        if let Some((_, previous_end)) = merged.last_mut()
            && start < *previous_end
        {
            continue;
        }
        merged.push((start, end));
    }
    merged
}

fn fold_case(value: &str) -> String {
    value.to_lowercase().replace('ς', "σ")
}

#[cfg(test)]
mod tests {
    use super::{RemoteEditor, case_insensitive_ranges};

    #[test]
    fn case_insensitive_replace_handles_lowercase_expanding_unicode() {
        let mut editor = RemoteEditor::new("unicode.txt".into(), "İx".into());
        editor.search_query = "i".into();
        editor.replace_query = "Z".into();

        assert!(editor.replace_next());
        assert_eq!(editor.content, "Zx");
    }

    #[test]
    fn case_insensitive_replace_all_preserves_unicode_boundaries() {
        let mut editor = RemoteEditor::new("unicode.txt".into(), "İ İ".into());
        editor.search_query = "i".into();
        editor.replace_query = "Z".into();

        assert_eq!(editor.replace_all(), 2);
        assert_eq!(editor.content, "Z Z");
    }

    #[test]
    fn case_insensitive_search_case_folds_greek_sigma_variants() {
        let mut editor = RemoteEditor::new("unicode.txt".into(), "ΟΣ ος ΟΣ".into());
        editor.search_query = "ς".into();

        assert_eq!(editor.count_search_matches(), 3);
    }

    #[test]
    fn case_insensitive_ranges_are_non_overlapping_after_unicode_expansion() {
        let ranges = case_insensitive_ranges("İİİ", "\u{307}i");

        assert_eq!(ranges, vec![(0, "İİ".len())]);
    }

    #[test]
    fn case_insensitive_replace_all_skips_overlapping_unicode_matches() {
        let mut editor = RemoteEditor::new("unicode.txt".into(), "İİİ".into());
        editor.search_query = "\u{307}i".into();
        editor.replace_query = "Z".into();

        assert_eq!(editor.replace_all(), 1);
        assert_eq!(editor.content, "Zİ");
    }
}
