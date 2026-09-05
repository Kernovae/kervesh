use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MacroStep {
    SendText { text: String, append_newline: bool },
    DelayMs(u64),
    ExpectPrompt(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationMacro {
    pub id: String,
    pub name: String,
    pub description: String,
    pub host_id: Option<String>,
    pub steps: Vec<MacroStep>,
    pub run_on_connect: bool,
}

impl AutomationMacro {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            description: description.into(),
            host_id: None,
            steps: Vec::new(),
            run_on_connect: false,
        }
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(!self.name.trim().is_empty(), "Macro name cannot be empty");
        for step in &self.steps {
            match step {
                MacroStep::SendText { text, .. } => {
                    ensure!(!text.is_empty(), "SendText step cannot have empty text");
                }
                MacroStep::DelayMs(ms) => {
                    ensure!(
                        *ms > 0 && *ms <= 60_000,
                        "Delay must be between 1ms and 60000ms"
                    );
                }
                MacroStep::ExpectPrompt(prompt) => {
                    ensure!(!prompt.is_empty(), "ExpectPrompt step cannot be empty");
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_automation_macro_validation() {
        let mut m = AutomationMacro::new("Deploy App", "Deploys production service");
        m.steps.push(MacroStep::SendText {
            text: "cd /opt/app".into(),
            append_newline: true,
        });
        m.steps.push(MacroStep::DelayMs(500));
        m.steps.push(MacroStep::ExpectPrompt("$".into()));
        assert!(m.validate().is_ok());

        let mut invalid = AutomationMacro::new("Bad Macro", "");
        invalid.steps.push(MacroStep::DelayMs(0));
        assert!(invalid.validate().is_err());
    }
}
