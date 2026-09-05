use crate::{AuthMethod, Host};
use uuid::Uuid;

#[derive(Default, Clone)]
struct HostConfig {
    hostname: Option<String>,
    user: Option<String>,
    port: Option<u16>,
    identity_file: Option<String>,
    keepalive: Option<u64>,
    timeout: Option<u64>,
}

pub fn default_ssh_config_path() -> Option<std::path::PathBuf> {
    directories::BaseDirs::new().map(|b| b.home_dir().join(".ssh").join("config"))
}

pub fn parse_ssh_config(content: &str) -> Vec<Host> {
    let mut defaults = HostConfig::default();
    let mut hosts: Vec<(String, HostConfig)> = Vec::new();
    let mut current_aliases: Vec<String> = Vec::new();
    let mut current_config = HostConfig::default();

    let flush_current = |aliases: &mut Vec<String>,
                         config: &mut HostConfig,
                         hosts: &mut Vec<(String, HostConfig)>,
                         defaults: &mut HostConfig| {
        if aliases.is_empty() {
            return;
        }
        for alias in aliases.drain(..) {
            if alias == "*" || alias.contains('*') || alias.contains('?') {
                // Merge into defaults
                if let Some(h) = &config.hostname {
                    defaults.hostname = Some(h.clone());
                }
                if let Some(u) = &config.user {
                    defaults.user = Some(u.clone());
                }
                if let Some(p) = config.port {
                    defaults.port = Some(p);
                }
                if let Some(i) = &config.identity_file {
                    defaults.identity_file = Some(i.clone());
                }
                if let Some(k) = config.keepalive {
                    defaults.keepalive = Some(k);
                }
                if let Some(t) = config.timeout {
                    defaults.timeout = Some(t);
                }
            } else {
                hosts.push((alias, config.clone()));
            }
        }
        *config = HostConfig::default();
    };

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Split key and value (supports space or '=' delimiter)
        let (key, value) = if let Some((k, v)) = line.split_once('=') {
            (k.trim(), v.trim())
        } else if let Some((k, v)) = line.split_once(char::is_whitespace) {
            (k.trim(), v.trim())
        } else {
            continue;
        };

        let key_lower = key.to_ascii_lowercase();

        if key_lower == "host" {
            flush_current(
                &mut current_aliases,
                &mut current_config,
                &mut hosts,
                &mut defaults,
            );
            current_aliases = value
                .split_whitespace()
                .map(|s| s.trim_matches('"').to_string())
                .collect();
            continue;
        }

        let val_clean = value.trim_matches('"').to_string();

        match key_lower.as_str() {
            "hostname" => current_config.hostname = Some(val_clean),
            "user" => current_config.user = Some(val_clean),
            "port" => {
                if let Ok(p) = val_clean.parse::<u16>() {
                    current_config.port = Some(p);
                }
            }
            "identityfile" => current_config.identity_file = Some(val_clean),
            "serveraliveinterval" => {
                if let Ok(k) = val_clean.parse::<u64>() {
                    current_config.keepalive = Some(k);
                }
            }
            "connecttimeout" => {
                if let Ok(t) = val_clean.parse::<u64>() {
                    current_config.timeout = Some(t);
                }
            }
            _ => {}
        }
    }

    flush_current(
        &mut current_aliases,
        &mut current_config,
        &mut hosts,
        &mut defaults,
    );

    let mut result = Vec::new();
    let home_dir = directories::BaseDirs::new()
        .map(|b| b.home_dir().to_string_lossy().to_string())
        .unwrap_or_else(|| std::env::var("HOME").unwrap_or_default());

    for (alias, cfg) in hosts {
        let hostname = cfg
            .hostname
            .or(defaults.hostname.clone())
            .unwrap_or_else(|| alias.clone());

        let username = cfg
            .user
            .or(defaults.user.clone())
            .unwrap_or_else(|| "root".to_string());

        let port = cfg.port.or(defaults.port).unwrap_or(22);

        let key_path_raw = cfg.identity_file.or(defaults.identity_file.clone());
        let key_path = match key_path_raw {
            Some(p) => {
                if p.starts_with("~/") && !home_dir.is_empty() {
                    format!("{home_dir}/{}", &p[2..])
                } else {
                    p
                }
            }
            None => String::new(),
        };

        let auth = if !key_path.is_empty() {
            AuthMethod::PrivateKey
        } else {
            AuthMethod::Password
        };

        let host = Host {
            terminal_profile: None,
            id: Uuid::new_v4().to_string(),
            name: alias,
            hostname,
            port,
            username,
            auth,
            key_path,
            group: "OpenSSH".to_string(),
            tags: "imported, openssh".to_string(),
            favorite: false,
            timeout_secs: cfg.timeout.or(defaults.timeout).unwrap_or(15),
            keepalive_secs: cfg.keepalive.or(defaults.keepalive).unwrap_or(30),
            auto_reconnect: false,
            last_connected: 0,
        };

        if host.validate().is_ok() {
            result.push(host);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_ssh_config() {
        let config = r#"
# Global defaults
Host *
    User devuser
    Port 2222
    ServerAliveInterval 60

Host web-server
    HostName 192.168.1.50
    User admin
    Port 22
    IdentityFile ~/.ssh/id_ed25519

Host db-prod db-staging
    HostName 10.0.0.1
    IdentityFile /keys/db.pem

Host bastion
    HostName bastion.example.com
"#;

        let hosts = parse_ssh_config(config);
        assert_eq!(hosts.len(), 4);

        let web = hosts.iter().find(|h| h.name == "web-server").unwrap();
        assert_eq!(web.hostname, "192.168.1.50");
        assert_eq!(web.username, "admin");
        assert_eq!(web.port, 22);
        assert_eq!(web.auth, AuthMethod::PrivateKey);
        assert!(web.key_path.ends_with(".ssh/id_ed25519"));
        assert_eq!(web.keepalive_secs, 60);

        let db_prod = hosts.iter().find(|h| h.name == "db-prod").unwrap();
        assert_eq!(db_prod.hostname, "10.0.0.1");
        assert_eq!(db_prod.username, "devuser");
        assert_eq!(db_prod.port, 2222);
        assert_eq!(db_prod.key_path, "/keys/db.pem");

        let db_staging = hosts.iter().find(|h| h.name == "db-staging").unwrap();
        assert_eq!(db_staging.hostname, "10.0.0.1");
        assert_eq!(db_staging.username, "devuser");

        let bastion = hosts.iter().find(|h| h.name == "bastion").unwrap();
        assert_eq!(bastion.hostname, "bastion.example.com");
        assert_eq!(bastion.username, "devuser");
        assert_eq!(bastion.port, 2222);
    }
}
