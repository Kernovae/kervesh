use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TunnelKind {
    Local,
    Remote,
    Dynamic, // SOCKS5
}

impl TunnelKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "Local (-L)",
            Self::Remote => "Remote (-R)",
            Self::Dynamic => "Dynamic SOCKS5 (-D)",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TunnelConfig {
    pub id: String,
    pub host_id: String,
    pub name: String,
    pub kind: TunnelKind,
    pub bind_addr: String,
    pub bind_port: u16,
    pub target_host: String,
    pub target_port: u16,
    pub auto_start: bool,
}

impl TunnelConfig {
    pub fn new(
        host_id: impl Into<String>,
        name: impl Into<String>,
        kind: TunnelKind,
        bind_port: u16,
        target_host: impl Into<String>,
        target_port: u16,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            host_id: host_id.into(),
            name: name.into(),
            kind,
            bind_addr: "127.0.0.1".into(),
            bind_port,
            target_host: target_host.into(),
            target_port,
            auto_start: false,
        }
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(!self.name.trim().is_empty(), "Tunnel name cannot be empty");
        ensure!(self.bind_port > 0, "Bind port must be > 0");
        if self.kind != TunnelKind::Dynamic {
            ensure!(
                !self.target_host.trim().is_empty(),
                "Target host cannot be empty"
            );
            ensure!(self.target_port > 0, "Target port must be > 0");
        }
        Ok(())
    }

    pub fn display_summary(&self) -> String {
        match self.kind {
            TunnelKind::Local => format!(
                "{}:{} ➔ {}:{}",
                self.bind_addr, self.bind_port, self.target_host, self.target_port
            ),
            TunnelKind::Remote => format!(
                "Remote :{} ➔ {}:{}",
                self.bind_port, self.target_host, self.target_port
            ),
            TunnelKind::Dynamic => format!("SOCKS5 on {}:{}", self.bind_addr, self.bind_port),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TunnelStats {
    pub bytes_rx: u64,
    pub bytes_tx: u64,
    pub active_connections: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tunnel_validation() {
        let t = TunnelConfig::new("h1", "Postgres", TunnelKind::Local, 5433, "127.0.0.1", 5432);
        assert!(t.validate().is_ok());
        assert_eq!(t.display_summary(), "127.0.0.1:5433 ➔ 127.0.0.1:5432");

        let socks = TunnelConfig::new("h1", "SocksProxy", TunnelKind::Dynamic, 1080, "", 0);
        assert!(socks.validate().is_ok());
        assert_eq!(socks.display_summary(), "SOCKS5 on 127.0.0.1:1080");

        let invalid = TunnelConfig::new("h1", "", TunnelKind::Local, 0, "", 0);
        assert!(invalid.validate().is_err());
    }
}
