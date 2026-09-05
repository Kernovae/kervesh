use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct X11ForwardingConfig {
    pub enabled: bool,
    pub display: String,
    pub trusted: bool,
    pub auth_protocol: String,
    pub auth_cookie: String,
    pub screen: u32,
}

impl Default for X11ForwardingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            display: Self::detect_local_display(),
            trusted: true,
            auth_protocol: "MIT-MAGIC-COOKIE-1".into(),
            auth_cookie: Self::generate_cookie(),
            screen: 0,
        }
    }
}

impl X11ForwardingConfig {
    pub fn detect_local_display() -> String {
        std::env::var("DISPLAY").unwrap_or_else(|_| {
            if cfg!(windows) {
                "127.0.0.1:0.0".into()
            } else {
                ":0".into()
            }
        })
    }

    pub fn generate_cookie() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let pid = std::process::id();
        format!(
            "{:016x}{:08x}{:08x}",
            nanos,
            pid,
            (nanos >> 32) ^ 0xA5A5A5A5
        )
    }

    pub fn parse_display_number(&self) -> (String, u32, u32) {
        let disp = self.display.trim();
        let (host_part, rest) = if let Some(colon) = disp.find(':') {
            (&disp[..colon], &disp[colon + 1..])
        } else {
            ("", disp)
        };

        let (display_num, screen_num) = if let Some(dot) = rest.find('.') {
            (
                rest[..dot].parse::<u32>().unwrap_or(0),
                rest[dot + 1..].parse::<u32>().unwrap_or(0),
            )
        } else {
            (rest.parse::<u32>().unwrap_or(0), 0)
        };

        (host_part.to_string(), display_num, screen_num)
    }

    pub fn local_x11_socket_path(&self) -> Option<String> {
        let (host, display_num, _) = self.parse_display_number();
        if host.is_empty() || host == "unix" {
            let path = format!("/tmp/.X11-unix/X{}", display_num);
            Some(path)
        } else {
            None
        }
    }

    pub fn local_x11_tcp_addr(&self) -> String {
        let (host, display_num, _) = self.parse_display_number();
        let target_host = if host.is_empty() || host == "unix" || host == "localhost" {
            "127.0.0.1"
        } else {
            &host
        };
        let target_port = 6000 + display_num;
        format!("{}:{}", target_host, target_port)
    }

    pub fn validate(&self) -> Result<()> {
        if self.enabled {
            ensure!(
                !self.display.trim().is_empty(),
                "X11 DISPLAY cannot be empty"
            );
            ensure!(
                !self.auth_protocol.trim().is_empty(),
                "X11 auth protocol cannot be empty"
            );
            ensure!(
                !self.auth_cookie.trim().is_empty(),
                "X11 auth cookie cannot be empty"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_x11_cookie_generation() {
        let c1 = X11ForwardingConfig::generate_cookie();
        let c2 = X11ForwardingConfig::generate_cookie();
        assert_eq!(c1.len(), 32);
        assert_eq!(c2.len(), 32);
        assert_ne!(c1, c2);
    }

    #[test]
    fn test_display_parsing() {
        let cfg = X11ForwardingConfig {
            display: ":1".into(),
            ..Default::default()
        };
        assert_eq!(cfg.parse_display_number(), ("".into(), 1, 0));
        assert_eq!(cfg.local_x11_tcp_addr(), "127.0.0.1:6001");
        assert_eq!(
            cfg.local_x11_socket_path(),
            Some("/tmp/.X11-unix/X1".into())
        );

        let cfg2 = X11ForwardingConfig {
            display: "192.168.1.10:2.1".into(),
            ..Default::default()
        };
        assert_eq!(cfg2.parse_display_number(), ("192.168.1.10".into(), 2, 1));
        assert_eq!(cfg2.local_x11_tcp_addr(), "192.168.1.10:6002");
        assert_eq!(cfg2.local_x11_socket_path(), None);
    }
}
