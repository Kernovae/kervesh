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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Host {
    pub terminal_profile: Option<String>,
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
    pub jump_host: Option<String>,
    pub proxy_jump: Option<String>,
    pub proxy_command: Option<String>,
    pub forward_agent: bool,
    pub local_forwards: Vec<String>,
    pub remote_forwards: Vec<String>,
    pub dynamic_forwards: Vec<u16>,
    pub thresholds: Option<crate::MonitorThresholds>,
    pub protocol: crate::protocol::ProtocolKind,
    pub serial_config: Option<crate::protocol::SerialConfig>,
    pub telnet_config: Option<crate::protocol::TelnetConfig>,
    pub ftp_config: Option<crate::protocol::FtpConfig>,
    pub rdp_config: Option<crate::protocol::RemoteDesktopConfig>,
    pub x11: Option<crate::x11::X11ForwardingConfig>,
    #[serde(skip)]
    pub last_connected: i64,
}
impl Default for Host {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            terminal_profile: None,
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
            jump_host: None,
            proxy_jump: None,
            proxy_command: None,
            forward_agent: false,
            local_forwards: Vec::new(),
            remote_forwards: Vec::new(),
            dynamic_forwards: Vec::new(),
            thresholds: None,
            protocol: crate::protocol::ProtocolKind::SSH,
            serial_config: None,
            telnet_config: None,
            ftp_config: None,
            rdp_config: None,
            x11: None,
            last_connected: 0,
        }
    }
}
impl Host {
    pub fn validate(&self) -> Result<()> {
        Uuid::parse_str(&self.id)?;
        if self.protocol == crate::protocol::ProtocolKind::Serial {
            ensure!(!self.name.trim().is_empty(), "Name is required");
            if let Some(cfg) = &self.serial_config {
                cfg.validate()?;
            }
        } else {
            for (label, value) in [("Name", &self.name), ("Host", &self.hostname)] {
                ensure!(!value.trim().is_empty(), "{label} is required");
                ensure!(
                    value.len() <= 255 && !value.chars().any(char::is_control),
                    "Invalid {label}"
                );
            }
            if self.protocol == crate::protocol::ProtocolKind::SSH {
                ensure!(!self.username.trim().is_empty(), "User is required");
            }
            ensure!(
                !self.hostname.chars().any(char::is_whitespace),
                "Host cannot contain whitespace"
            );
            ensure!(self.port > 0, "Port must be between 1 and 65535");
        }
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
    pub fn tag_list(&self) -> Vec<&str> {
        self.tags
            .split([',', ' ', ';'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(from = "SettingsWire")]
pub struct Settings {
    pub terminal_profiles: Vec<crate::TerminalProfile>,
    pub default_terminal_profile: String,
    pub dark: bool,
    pub font_size: f32,
    pub scrollback: usize,
    pub monitor_secs: u64,
    pub show_hidden: bool,
    pub sidebar: bool,
    pub sftp_panel: bool,
    pub default_thresholds: crate::MonitorThresholds,
    pub keybindings: crate::KeyBindingsConfig,
    pub cpu_interval_secs: u64,
    pub memory_interval_secs: u64,
    pub disk_interval_secs: u64,
    pub network_interval_secs: u64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            terminal_profiles: crate::TerminalProfile::builtins(),
            default_terminal_profile: "default".into(),
            dark: true,
            font_size: 14.0,
            scrollback: 10000,
            monitor_secs: 2,
            show_hidden: false,
            sidebar: true,
            sftp_panel: true,
            default_thresholds: crate::MonitorThresholds::default(),
            keybindings: crate::KeyBindingsConfig::default(),
            cpu_interval_secs: 2,
            memory_interval_secs: 2,
            disk_interval_secs: 10,
            network_interval_secs: 2,
        }
    }
}
impl Settings {
    pub fn terminal_profile(&self, id: Option<&str>) -> &crate::TerminalProfile {
        self.terminal_profiles
            .iter()
            .find(|p| Some(p.id.as_str()) == id)
            .or_else(|| {
                self.terminal_profiles
                    .iter()
                    .find(|p| p.id == self.default_terminal_profile)
            })
            .unwrap_or(&self.terminal_profiles[0])
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.terminal_profiles.is_empty() && self.terminal_profiles.len() <= 64,
            "Require 1–64 terminal profiles"
        );
        let mut ids = std::collections::HashSet::new();
        for profile in &self.terminal_profiles {
            profile.validate()?;
            ensure!(ids.insert(&profile.id), "Duplicate terminal profile ID");
        }
        ensure!(
            ids.contains(&self.default_terminal_profile),
            "Default terminal profile missing"
        );
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
        ensure!(
            (1..=300).contains(&self.cpu_interval_secs)
                && (1..=300).contains(&self.memory_interval_secs)
                && (1..=300).contains(&self.disk_interval_secs)
                && (1..=300).contains(&self.network_interval_secs),
            "Metric intervals must be 1–300 seconds"
        );
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SettingsWire {
    dark: bool,
    font_size: f32,
    scrollback: usize,
    monitor_secs: u64,
    show_hidden: bool,
    sidebar: bool,
    sftp_panel: bool,
    terminal_profiles: Option<Vec<crate::TerminalProfile>>,
    default_terminal_profile: String,
    default_thresholds: Option<crate::MonitorThresholds>,
    keybindings: Option<crate::KeyBindingsConfig>,
    cpu_interval_secs: Option<u64>,
    memory_interval_secs: Option<u64>,
    disk_interval_secs: Option<u64>,
    network_interval_secs: Option<u64>,
}
impl Default for SettingsWire {
    fn default() -> Self {
        let s = Settings::default();
        Self {
            dark: s.dark,
            font_size: s.font_size,
            scrollback: s.scrollback,
            monitor_secs: s.monitor_secs,
            show_hidden: s.show_hidden,
            sidebar: s.sidebar,
            sftp_panel: s.sftp_panel,
            terminal_profiles: None,
            default_terminal_profile: s.default_terminal_profile,
            default_thresholds: None,
            keybindings: None,
            cpu_interval_secs: Some(s.cpu_interval_secs),
            memory_interval_secs: Some(s.memory_interval_secs),
            disk_interval_secs: Some(s.disk_interval_secs),
            network_interval_secs: Some(s.network_interval_secs),
        }
    }
}
impl From<SettingsWire> for Settings {
    fn from(s: SettingsWire) -> Self {
        let terminal_profiles = s.terminal_profiles.unwrap_or_else(|| {
            let mut profiles = crate::TerminalProfile::builtins();
            profiles[0].font_size = s.font_size;
            profiles[0].scrollback = s.scrollback;
            profiles
        });
        Self {
            dark: s.dark,
            font_size: s.font_size,
            scrollback: s.scrollback,
            monitor_secs: s.monitor_secs,
            show_hidden: s.show_hidden,
            sidebar: s.sidebar,
            sftp_panel: s.sftp_panel,
            terminal_profiles,
            default_terminal_profile: s.default_terminal_profile,
            default_thresholds: s.default_thresholds.unwrap_or_default(),
            keybindings: s.keybindings.unwrap_or_default(),
            cpu_interval_secs: s.cpu_interval_secs.unwrap_or(2),
            memory_interval_secs: s.memory_interval_secs.unwrap_or(2),
            disk_interval_secs: s.disk_interval_secs.unwrap_or(10),
            network_interval_secs: s.network_interval_secs.unwrap_or(2),
        }
    }
}
