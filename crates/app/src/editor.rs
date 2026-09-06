use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_SAVE_OPERATION_ID: AtomicU64 = AtomicU64::new(1);

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
    pub close_prompt: bool,
    close_after_save: bool,
    revision: u64,
    pending_save_revision: Option<u64>,
    pending_save_path: Option<String>,
    pending_save_id: Option<u64>,
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
            close_prompt: false,
            close_after_save: false,
            revision: 0,
            pending_save_revision: None,
            pending_save_path: None,
            pending_save_id: None,
        }
    }

    pub fn mark_changed(&mut self) {
        self.dirty = true;
        self.revision = self.revision.saturating_add(1);
    }

    pub fn begin_save(&mut self) -> Option<(String, String, u64)> {
        self.begin_save_to(self.path.clone())
    }

    pub fn begin_save_to(&mut self, path: String) -> Option<(String, String, u64)> {
        let operation_id = NEXT_SAVE_OPERATION_ID.fetch_add(1, Ordering::Relaxed);
        self.begin_save_to_with_id(path, operation_id)
    }

    pub fn begin_save_to_with_id(
        &mut self,
        path: String,
        operation_id: u64,
    ) -> Option<(String, String, u64)> {
        if !self.dirty || self.saving {
            return None;
        }
        self.saving = true;
        self.error = None;
        self.pending_save_revision = Some(self.revision);
        self.pending_save_path = Some(path.clone());
        self.pending_save_id = Some(operation_id);
        Some((path, self.prepare_save_content(), operation_id))
    }

    pub fn complete_save(&mut self, path: &str, operation_id: u64) -> bool {
        if self.pending_save_path.as_deref() != Some(path)
            || self.pending_save_id != Some(operation_id)
        {
            return false;
        }
        let saved_revision = self.pending_save_revision.take();
        self.pending_save_path = None;
        self.pending_save_id = None;
        self.saving = false;
        self.path = path.to_owned();
        self.name = path.rsplit('/').next().unwrap_or(path).to_owned();
        self.save_as_input = path.to_owned();
        if saved_revision == Some(self.revision) {
            self.dirty = false;
            self.original_content = self.content.clone();
        } else {
            self.dirty = true;
            self.close_after_save = false;
        }
        true
    }

    pub fn fail_save(&mut self, path: &str, operation_id: u64, error: String) -> bool {
        if self.pending_save_path.as_deref() != Some(path)
            || self.pending_save_id != Some(operation_id)
        {
            return false;
        }
        self.pending_save_revision = None;
        self.pending_save_path = None;
        self.pending_save_id = None;
        self.saving = false;
        self.dirty = true;
        self.close_after_save = false;
        self.error = Some(error);
        true
    }

    pub fn request_close_after_save(&mut self) -> Option<(String, String, u64)> {
        self.close_after_save = true;
        self.begin_save()
    }

    pub fn take_close_after_save(&mut self) -> bool {
        std::mem::take(&mut self.close_after_save)
    }

    pub fn pending_save_path(&self) -> Option<&str> {
        self.pending_save_path.as_deref()
    }

    pub fn pending_save(&self) -> Option<(&str, u64)> {
        Some((self.pending_save_path.as_deref()?, self.pending_save_id?))
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
            self.mark_changed();
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
                let mut result = String::with_capacity(self.content.len());
                let mut cursor = 0;
                for (start, end) in ranges {
                    result.push_str(&self.content[cursor..start]);
                    result.push_str(&self.replace_query);
                    cursor = end;
                }
                result.push_str(&self.content[cursor..]);
                self.content = result;
            }
            self.mark_changed();
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
    let mut lower_byte_ranges: Vec<(usize, usize)> = Vec::with_capacity(content.len());
    for (start, ch) in content.char_indices() {
        let end = start + ch.len_utf8();
        for lower_ch in fold_case(&ch.to_string()).chars() {
            let lower_bytes = lower_ch.len_utf8();
            lower_content.push(lower_ch);
            lower_byte_ranges.extend(std::iter::repeat_n((start, end), lower_bytes));
        }
    }

    let mut ranges = lower_content
        .match_indices(&lower_query)
        .filter_map(|(lower_start, matched)| {
            let lower_end = lower_start + matched.len();
            let start = lower_byte_ranges.get(lower_start)?.0;
            let end = lower_byte_ranges.get(lower_end.checked_sub(1)?)?.1;
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

    #[test]
    fn save_completion_preserves_edit_made_while_write_was_pending() {
        let mut editor = RemoteEditor::new("remote.txt".into(), "before".into());
        editor.content = "saved".into();
        editor.mark_changed();
        let snapshot = editor.begin_save().expect("dirty editor should start save");

        editor.content.push_str(" and edited");
        editor.mark_changed();

        assert!(editor.complete_save(&snapshot.0, snapshot.2));
        assert!(editor.dirty);
        assert_eq!(editor.original_content, "before");
        assert_eq!(editor.content, "saved and edited");
    }

    #[test]
    fn save_completion_requires_matching_path() {
        let mut editor = RemoteEditor::new("remote.txt".into(), "before".into());
        editor.content = "saved".into();
        editor.mark_changed();
        let snapshot = editor.begin_save().expect("dirty editor should start save");

        assert!(!editor.complete_save("other.txt", snapshot.2));
        assert!(editor.saving);
        assert!(editor.dirty);
    }

    #[test]
    fn close_after_save_is_consumed_only_after_success() {
        let mut editor = RemoteEditor::new("remote.txt".into(), "before".into());
        editor.content = "saved".into();
        editor.mark_changed();
        let snapshot = editor
            .request_close_after_save()
            .expect("dirty editor should start close save");

        assert!(editor.complete_save(&snapshot.0, snapshot.2));
        assert!(editor.take_close_after_save());
        assert!(!editor.dirty);
    }

    #[test]
    fn save_as_changes_editor_path_only_after_success() {
        let mut editor = RemoteEditor::new("old.txt".into(), "before".into());
        editor.content = "saved".into();
        editor.mark_changed();
        let snapshot = editor
            .begin_save_to("new.txt".into())
            .expect("dirty editor should start save as");

        assert_eq!(editor.path, "old.txt");
        assert!(editor.fail_save(&snapshot.0, snapshot.2, "write failed".into()));
        assert_eq!(editor.path, "old.txt");
        assert!(editor.dirty);

        let snapshot = editor
            .begin_save_to("new.txt".into())
            .expect("failed save should be retryable");
        assert!(editor.complete_save(&snapshot.0, snapshot.2));
        assert_eq!(editor.path, "new.txt");
        assert!(!editor.dirty);
    }

    #[test]
    fn stale_write_completion_does_not_settle_a_new_save() {
        let mut editor = RemoteEditor::new("remote.txt".into(), "before".into());
        editor.content = "first".into();
        editor.mark_changed();
        let old_save = editor.begin_save().expect("first save should start");
        assert!(editor.fail_save(&old_save.0, old_save.2, "discarded".into()));

        editor.content = "second".into();
        editor.mark_changed();
        let new_save = editor.begin_save().expect("retry save should start");
        assert_ne!(old_save.2, new_save.2);
        assert!(!editor.complete_save(&old_save.0, old_save.2));
        assert!(editor.saving);
        assert!(editor.dirty);
        assert!(editor.complete_save(&new_save.0, new_save.2));
        assert!(!editor.dirty);
    }
}
