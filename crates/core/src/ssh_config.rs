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
    proxy_jump: Option<String>,
    proxy_command: Option<String>,
    forward_agent: Option<bool>,
    local_forwards: Vec<String>,
    remote_forwards: Vec<String>,
    dynamic_forwards: Vec<u16>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpenSshImportReport {
    pub hosts: Vec<Host>,
    pub unsupported_directives: Vec<(String, String)>,
    pub proxy_jump_count: usize,
    pub forwarded_rules_count: usize,
}

pub fn default_ssh_config_path() -> Option<std::path::PathBuf> {
    directories::BaseDirs::new().map(|b| b.home_dir().join(".ssh").join("config"))
}

pub fn parse_ssh_config(content: &str) -> Vec<Host> {
    parse_ssh_config_with_report(content).hosts
}

pub fn parse_ssh_config_with_report(content: &str) -> OpenSshImportReport {
    let mut defaults = HostConfig::default();
    let mut hosts: Vec<(String, HostConfig)> = Vec::new();
    let mut current_aliases: Vec<String> = Vec::new();
    let mut current_config = HostConfig::default();
    let mut unsupported = Vec::new();

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
                if let Some(pj) = &config.proxy_jump {
                    defaults.proxy_jump = Some(pj.clone());
                }
                if let Some(pc) = &config.proxy_command {
                    defaults.proxy_command = Some(pc.clone());
                }
                if let Some(fa) = config.forward_agent {
                    defaults.forward_agent = Some(fa);
                }
                defaults
                    .local_forwards
                    .extend(config.local_forwards.clone());
                defaults
                    .remote_forwards
                    .extend(config.remote_forwards.clone());
                defaults
                    .dynamic_forwards
                    .extend(config.dynamic_forwards.clone());
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
            "proxyjump" => current_config.proxy_jump = Some(val_clean),
            "proxycommand" => current_config.proxy_command = Some(val_clean),
            "forwardagent" => {
                current_config.forward_agent =
                    Some(val_clean.eq_ignore_ascii_case("yes") || val_clean == "1");
            }
            "localforward" => current_config.local_forwards.push(val_clean),
            "remoteforward" => current_config.remote_forwards.push(val_clean),
            "dynamicforward" => {
                if let Ok(p) = val_clean.parse::<u16>() {
                    current_config.dynamic_forwards.push(p);
                }
            }
            _ => {
                let context = current_aliases.join(" ");
                let host_label = if context.is_empty() {
                    "global"
                } else {
                    &context
                };
                unsupported.push((host_label.to_string(), key.to_string()));
            }
        }
    }

    flush_current(
        &mut current_aliases,
        &mut current_config,
        &mut hosts,
        &mut defaults,
    );

    let mut result_hosts = Vec::new();
    let home_dir = directories::BaseDirs::new()
        .map(|b| b.home_dir().to_string_lossy().to_string())
        .unwrap_or_else(|| std::env::var("HOME").unwrap_or_default());

    let mut proxy_jump_count = 0;
    let mut forwarded_rules_count = 0;

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

        let proxy_jump = cfg.proxy_jump.or(defaults.proxy_jump.clone());
        if proxy_jump.is_some() {
            proxy_jump_count += 1;
        }
        let proxy_command = cfg.proxy_command.or(defaults.proxy_command.clone());
        let forward_agent = cfg
            .forward_agent
            .or(defaults.forward_agent)
            .unwrap_or(false);

        let mut local_forwards = defaults.local_forwards.clone();
        local_forwards.extend(cfg.local_forwards);

        let mut remote_forwards = defaults.remote_forwards.clone();
        remote_forwards.extend(cfg.remote_forwards);

        let mut dynamic_forwards = defaults.dynamic_forwards.clone();
        dynamic_forwards.extend(cfg.dynamic_forwards);

        forwarded_rules_count +=
            local_forwards.len() + remote_forwards.len() + dynamic_forwards.len();

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
            jump_host: None,
            proxy_jump,
            proxy_command,
            forward_agent,
            local_forwards,
            remote_forwards,
            dynamic_forwards,
            thresholds: None,
            last_connected: 0,
            ..Host::default()
        };

        if host.validate().is_ok() {
            result_hosts.push(host);
        }
    }

    OpenSshImportReport {
        hosts: result_hosts,
        unsupported_directives: unsupported,
        proxy_jump_count,
        forwarded_rules_count,
    }
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

    #[test]
    fn test_parse_advanced_ssh_config_with_proxy_jump_and_forwards() {
        let config = r#"
Host bastion.corp
    HostName bastion.internal.net
    User bastion-admin
    Port 2222

Host internal-db
    HostName 10.100.0.5
    User postgres
    ProxyJump bastion-admin@bastion.corp:2222
    LocalForward 5432 127.0.0.1:5432
    RemoteForward 9000 127.0.0.1:9000
    DynamicForward 1080
    ForwardAgent yes
    GSSAPIAuthentication yes
"#;

        let report = parse_ssh_config_with_report(config);
        assert_eq!(report.hosts.len(), 2);
        assert_eq!(report.proxy_jump_count, 1);
        assert_eq!(report.forwarded_rules_count, 3);

        let db = report
            .hosts
            .iter()
            .find(|h| h.name == "internal-db")
            .unwrap();
        assert_eq!(
            db.proxy_jump.as_deref(),
            Some("bastion-admin@bastion.corp:2222")
        );
        assert!(db.forward_agent);
        assert_eq!(db.local_forwards, vec!["5432 127.0.0.1:5432"]);
        assert_eq!(db.remote_forwards, vec!["9000 127.0.0.1:9000"]);
        assert_eq!(db.dynamic_forwards, vec![1080]);

        assert!(
            report
                .unsupported_directives
                .iter()
                .any(|(_ctx, dir)| dir.eq_ignore_ascii_case("gssapiauthentication"))
        );
    }
}
