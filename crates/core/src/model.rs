use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthMethod {
    #[default]
    Password,
    PrivateKey,
    Agent,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Host {
    pub id: String,
    pub name: String,
    pub hostname: String,
    pub port: u16,
    pub username: String,
    pub auth: AuthMethod,
    pub key_path: String,
    pub group: String,
    pub tags: String,
    pub favorite: bool,
    pub timeout_secs: u64,
    pub keepalive_secs: u64,
    pub auto_reconnect: bool,
    #[serde(skip)]
    pub last_connected: i64,
}
impl Default for Host {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: String::new(),
            hostname: String::new(),
            port: 22,
            username: String::new(),
            auth: AuthMethod::Password,
            key_path: String::new(),
            group: String::new(),
            tags: String::new(),
            favorite: false,
            timeout_secs: 15,
            keepalive_secs: 30,
            auto_reconnect: false,
            last_connected: 0,
        }
    }
}
impl Host {
    pub fn validate(&self) -> Result<()> {
        Uuid::parse_str(&self.id)?;
        for (label, value) in [
            ("Name", &self.name),
            ("Host", &self.hostname),
            ("User", &self.username),
        ] {
            ensure!(!value.trim().is_empty(), "{label} is required");
            ensure!(
                value.len() <= 255 && !value.chars().any(char::is_control),
                "Invalid {label}"
            );
        }
        ensure!(
            !self.hostname.chars().any(char::is_whitespace),
            "Host cannot contain whitespace"
        );
        ensure!(self.port > 0, "Port must be between 1 and 65535");
        ensure!(
            (1..=300).contains(&self.timeout_secs),
            "Timeout must be 1–300 seconds"
        );
        ensure!(
            self.keepalive_secs <= 3600,
            "Keepalive must be 0–3600 seconds"
        );
        ensure!(
            self.auth != AuthMethod::PrivateKey || !self.key_path.is_empty(),
            "Private key path is required"
        );
        ensure!(
            self.group.len() <= 255 && self.tags.len() <= 2048,
            "Group or tags too long"
        );
        Ok(())
    }
    pub fn duplicate(&self) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: format!("{} copy", self.name),
            last_connected: 0,
            ..self.clone()
        }
    }
    pub fn matches(&self, query: &str) -> bool {
        format!(
            "{} {} {} {} {}",
            self.name, self.hostname, self.username, self.group, self.tags
        )
        .to_lowercase()
        .contains(&query.to_lowercase())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub dark: bool,
    pub font_size: f32,
    pub scrollback: usize,
    pub monitor_secs: u64,
    pub show_hidden: bool,
    pub sidebar: bool,
    pub sftp_panel: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            dark: true,
            font_size: 14.0,
            scrollback: 10000,
            monitor_secs: 2,
            show_hidden: false,
            sidebar: true,
            sftp_panel: true,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.font_size.is_finite() && (8.0..=32.0).contains(&self.font_size),
            "Font size must be 8–32"
        );
        ensure!(
            self.scrollback <= 100000,
            "Scrollback limit is 100000 lines"
        );
        ensure!(
            (1..=300).contains(&self.monitor_secs),
            "Monitor interval must be 1–300 seconds"
        );
        Ok(())
    }
}
