use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerAction {
    Notification(String),
    SendInput(String),
    Highlight(String),
    PlayBeep,
}

impl TriggerAction {
    pub fn display_name(&self) -> String {
        match self {
            TriggerAction::Notification(msg) => format!("Notify: \"{}\"", msg),
            TriggerAction::SendInput(input) => format!("Send: \"{}\"", input),
            TriggerAction::Highlight(color) => format!("Highlight: {}", color),
            TriggerAction::PlayBeep => "Play Alert Sound".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerRule {
    pub id: String,
    pub name: String,
    pub pattern: String,
    pub is_regex: bool,
    pub case_sensitive: bool,
    pub action: TriggerAction,
    pub enabled: bool,
    pub host_id: Option<String>,
}

impl TriggerRule {
    pub fn new(
        name: impl Into<String>,
        pattern: impl Into<String>,
        is_regex: bool,
        action: TriggerAction,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            pattern: pattern.into(),
            is_regex,
            case_sensitive: false,
            action,
            enabled: true,
            host_id: None,
        }
    }
}

pub struct TriggerEngine {
    compiled_rules: Vec<(TriggerRule, Option<Regex>)>,
}

impl TriggerEngine {
    pub fn new(rules: &[TriggerRule]) -> Self {
        let mut compiled = Vec::new();
        for r in rules {
            let regex = if r.is_regex {
                RegexBuilder::new(&r.pattern)
                    .case_insensitive(!r.case_sensitive)
                    .build()
                    .ok()
            } else {
                None
            };
            compiled.push((r.clone(), regex));
        }
        Self {
            compiled_rules: compiled,
        }
    }

    pub fn evaluate(&self, text: &str, host_id: Option<&str>) -> Vec<TriggerAction> {
        let mut triggered = Vec::new();
        for (rule, regex_opt) in &self.compiled_rules {
            if !rule.enabled {
                continue;
            }
            if let (Some(target_host), Some(current_host)) = (&rule.host_id, host_id) {
                if target_host != current_host {
                    continue;
                }
            } else if rule.host_id.is_some() && host_id.is_none() {
                continue;
            }

            let matches = if let Some(regex) = regex_opt {
                regex.is_match(text)
            } else if rule.case_sensitive {
                text.contains(&rule.pattern)
            } else {
                text.to_lowercase().contains(&rule.pattern.to_lowercase())
            };

            if matches {
                triggered.push(rule.action.clone());
            }
        }
        triggered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trigger_engine_substring() {
        let rule1 = TriggerRule::new(
            "Disk Full Alert",
            "No space left on device",
            false,
            TriggerAction::Notification("Disk is full on server!".into()),
        );
        let rule2 = TriggerRule::new(
            "Password Prompt",
            "password for",
            false,
            TriggerAction::SendInput("mypassword\n".into()),
        );

        let engine = TriggerEngine::new(&[rule1, rule2]);

        let actions = engine.evaluate("bash: write error: No space left on device", None);
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            TriggerAction::Notification("Disk is full on server!".into())
        );

        let actions2 = engine.evaluate("[sudo] password for user:", None);
        assert_eq!(actions2.len(), 1);
        assert_eq!(actions2[0], TriggerAction::SendInput("mypassword\n".into()));
    }

    #[test]
    fn test_trigger_engine_regex() {
        let rule = TriggerRule::new(
            "Error Code Matcher",
            r"ERROR:\s+code\s+\d+",
            true,
            TriggerAction::PlayBeep,
        );

        let engine = TriggerEngine::new(&[rule]);
        assert_eq!(
            engine.evaluate("2026-09-05 ERROR: code 500 in service", None),
            vec![TriggerAction::PlayBeep]
        );
        assert_eq!(engine.evaluate("Everything is OK", None), vec![]);
    }
}
