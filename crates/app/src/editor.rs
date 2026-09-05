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
            let lower_content = self.content.to_lowercase();
            let lower_query = self.search_query.to_lowercase();
            lower_content.matches(&lower_query).count()
        }
    }

    pub fn replace_next(&mut self) -> bool {
        if self.search_query.is_empty() {
            return false;
        }
        if let Some(idx) = if self.case_sensitive {
            self.content.find(&self.search_query)
        } else {
            let lower_content = self.content.to_lowercase();
            let lower_query = self.search_query.to_lowercase();
            lower_content.find(&lower_query)
        } {
            self.content
                .replace_range(idx..idx + self.search_query.len(), &self.replace_query);
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
                let lower_query = self.search_query.to_lowercase();
                let mut result = String::with_capacity(self.content.len());
                let mut last_idx = 0;
                let lower_content = self.content.to_lowercase();
                for (start_idx, _) in lower_content.match_indices(&lower_query) {
                    result.push_str(&self.content[last_idx..start_idx]);
                    result.push_str(&self.replace_query);
                    last_idx = start_idx + self.search_query.len();
                }
                result.push_str(&self.content[last_idx..]);
                self.content = result;
            }
            self.dirty = true;
        }
        count
    }
}
