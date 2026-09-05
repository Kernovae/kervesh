use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchQuery {
    pub directory: String,
    pub pattern: String,
    pub extension: Option<String>,
    pub case_sensitive: bool,
    pub is_regex: bool,
    pub max_results: usize,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            directory: "/".into(),
            pattern: String::new(),
            extension: None,
            case_sensitive: false,
            is_regex: false,
            max_results: 200,
        }
    }
}

impl SearchQuery {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.pattern.trim().is_empty(),
            "Search pattern cannot be empty"
        );
        ensure!(
            !self.directory.trim().is_empty(),
            "Search directory cannot be empty"
        );
        Ok(())
    }

    pub fn to_grep_command(&self) -> String {
        let mut flags = String::from("-rn");
        if !self.case_sensitive {
            flags.push('i');
        }
        if self.is_regex {
            flags.push('E');
        } else {
            flags.push('F');
        }

        let include_arg = if let Some(ext) = &self.extension {
            let clean = ext.trim().trim_start_matches('.');
            if clean.is_empty() {
                String::new()
            } else {
                format!(" --include=*.{}", clean)
            }
        } else {
            String::new()
        };

        let escaped_pattern = self.pattern.replace('\'', "'\\''");
        let dir = self.directory.trim_end_matches('/');
        let target_dir = if dir.is_empty() { "/" } else { dir };

        format!(
            "grep {} --exclude-dir=.git --exclude-dir=node_modules --exclude-dir=target{} -e '{}' '{}' 2>/dev/null | head -n {}",
            flags, include_arg, escaped_pattern, target_dir, self.max_results
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    pub path: String,
    pub line_number: usize,
    pub line_content: String,
}

impl SearchResult {
    pub fn parse_grep_output(output: &str) -> Vec<Self> {
        let mut results = Vec::new();
        for line in output.lines() {
            let mut parts = line.splitn(3, ':');
            if let (Some(path), Some(line_num_str), Some(content)) =
                (parts.next(), parts.next(), parts.next())
                && let Ok(line_num) = line_num_str.parse::<usize>()
            {
                results.push(Self {
                    path: path.to_string(),
                    line_number: line_num,
                    line_content: content.trim_end_matches('\r').to_string(),
                });
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_query_generation() {
        let q = SearchQuery {
            directory: "/var/log".into(),
            pattern: "ERROR".into(),
            extension: Some("log".into()),
            case_sensitive: true,
            is_regex: false,
            max_results: 50,
        };
        assert!(q.validate().is_ok());
        let cmd = q.to_grep_command();
        assert!(cmd.contains("-rnF"));
        assert!(cmd.contains("--include=*.log"));
        assert!(cmd.contains("head -n 50"));
    }

    #[test]
    fn test_search_output_parsing() {
        let out = "/etc/nginx/nginx.conf:15:    server_name example.com;\n/etc/nginx/nginx.conf:22:    listen 80;\n";
        let res = SearchResult::parse_grep_output(out);
        assert_eq!(res.len(), 2);
        assert_eq!(res[0].path, "/etc/nginx/nginx.conf");
        assert_eq!(res[0].line_number, 15);
        assert_eq!(res[0].line_content, "    server_name example.com;");
    }
}
