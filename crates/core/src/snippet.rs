use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snippet {
    pub id: String,
    pub name: String,
    pub command: String,
    pub description: String,
    pub tags: String,
}

impl Snippet {
    pub fn new(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            command: command.into(),
            description: String::new(),
            tags: String::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(!self.name.trim().is_empty(), "Snippet name cannot be empty");
        ensure!(
            !self.command.trim().is_empty(),
            "Snippet command cannot be empty"
        );
        ensure!(self.name.len() <= 128, "Snippet name too long");
        ensure!(self.command.len() <= 65536, "Snippet command too long");
        ensure!(
            self.description.len() <= 1024,
            "Snippet description too long"
        );
        ensure!(self.tags.len() <= 512, "Snippet tags too long");
        Ok(())
    }

    pub fn extract_placeholders(&self) -> Vec<String> {
        let mut placeholders = Vec::new();
        let mut rest = self.command.as_str();
        while let Some(start_idx) = rest.find("{{") {
            let after_start = &rest[start_idx + 2..];
            if let Some(end_idx) = after_start.find("}}") {
                let var_name = after_start[..end_idx].trim().to_string();
                if !var_name.is_empty() && !placeholders.contains(&var_name) {
                    placeholders.push(var_name);
                }
                rest = &after_start[end_idx + 2..];
            } else {
                break;
            }
        }
        placeholders
    }

    pub fn render(&self, values: &HashMap<String, String>) -> String {
        let mut rendered = self.command.clone();
        for (k, v) in values {
            let placeholder = format!("{{{{{}}}}}", k);
            rendered = rendered.replace(&placeholder, v);
            let placeholder_spaced = format!("{{{{ {} }}}}", k);
            rendered = rendered.replace(&placeholder_spaced, v);
        }
        rendered
    }

    pub fn matches(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.name.to_lowercase().contains(&q)
            || self.command.to_lowercase().contains(&q)
            || self.description.to_lowercase().contains(&q)
            || self.tags.to_lowercase().contains(&q)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snippet_placeholders_and_render() {
        let s = Snippet::new(
            "Restart Service",
            "sudo systemctl restart {{service_name}} && sudo journalctl -u {{service_name}} -n {{lines}}",
        );
        let vars = s.extract_placeholders();
        assert_eq!(vars, vec!["service_name", "lines"]);

        let mut values = HashMap::new();
        values.insert("service_name".into(), "nginx".into());
        values.insert("lines".into(), "50".into());

        let rendered = s.render(&values);
        assert_eq!(
            rendered,
            "sudo systemctl restart nginx && sudo journalctl -u nginx -n 50"
        );
    }

    #[test]
    fn test_snippet_validation() {
        let empty = Snippet::new("", "");
        assert!(empty.validate().is_err());

        let valid = Snippet::new("Tail logs", "tail -f /var/log/syslog");
        assert!(valid.validate().is_ok());
    }
}
