use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolKind {
    #[default]
    SSH,
    Telnet,
    Serial,
    FTP,
    RDP,
    VNC,
}

impl ProtocolKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::SSH => "SSH",
            Self::Telnet => "Telnet",
            Self::Serial => "Serial Port (UART)",
            Self::FTP => "FTP / FTPS",
            Self::RDP => "RDP (Remote Desktop)",
            Self::VNC => "VNC Remote Desktop",
        }
    }

    pub fn default_port(&self) -> u16 {
        match self {
            Self::SSH => 22,
            Self::Telnet => 23,
            Self::Serial => 0,
            Self::FTP => 21,
            Self::RDP => 3389,
            Self::VNC => 5900,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SerialParity {
    #[default]
    None,
    Odd,
    Even,
    Mark,
    Space,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SerialFlowControl {
    #[default]
    None,
    Software,
    Hardware,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerialConfig {
    pub port: String,
    pub baud_rate: u32,
    pub data_bits: u8,
    pub parity: SerialParity,
    pub stop_bits: u8,
    pub flow_control: SerialFlowControl,
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            port: if cfg!(target_os = "windows") {
                "COM1".into()
            } else {
                "/dev/ttyUSB0".into()
            },
            baud_rate: 115200,
            data_bits: 8,
            parity: SerialParity::None,
            stop_bits: 1,
            flow_control: SerialFlowControl::None,
        }
    }
}

impl SerialConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(!self.port.trim().is_empty(), "Serial port cannot be empty");
        ensure!(self.baud_rate > 0, "Baud rate must be greater than 0");
        ensure!(
            (5..=8).contains(&self.data_bits),
            "Data bits must be between 5 and 8"
        );
        ensure!(
            self.stop_bits == 1 || self.stop_bits == 2,
            "Stop bits must be 1 or 2"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelnetConfig {
    pub host: String,
    pub port: u16,
    pub terminal_type: String,
    pub naws: bool,
}

impl Default for TelnetConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 23,
            terminal_type: "xterm-256color".into(),
            naws: true,
        }
    }
}

impl TelnetConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(!self.host.trim().is_empty(), "Telnet host cannot be empty");
        ensure!(self.port > 0, "Port must be greater than 0");
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FtpTlsMode {
    #[default]
    None,
    Explicit,
    Implicit,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub tls_mode: FtpTlsMode,
    pub passive_mode: bool,
}

impl Default for FtpConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 21,
            username: "anonymous".into(),
            tls_mode: FtpTlsMode::None,
            passive_mode: true,
        }
    }
}

impl FtpConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(!self.host.trim().is_empty(), "FTP host cannot be empty");
        ensure!(self.port > 0, "FTP port must be greater than 0");
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteDesktopKind {
    #[default]
    Rdp,
    Vnc,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteDesktopConfig {
    pub kind: RemoteDesktopKind,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub domain: Option<String>,
    pub width: u32,
    pub height: u32,
    pub color_depth: u8,
    pub fullscreen: bool,
    pub custom_args: Vec<String>,
}

impl Default for RemoteDesktopConfig {
    fn default() -> Self {
        Self {
            kind: RemoteDesktopKind::Rdp,
            host: String::new(),
            port: 3389,
            username: String::new(),
            domain: None,
            width: 1920,
            height: 1080,
            color_depth: 24,
            fullscreen: false,
            custom_args: Vec::new(),
        }
    }
}

impl RemoteDesktopConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(!self.host.trim().is_empty(), "Host cannot be empty");
        ensure!(self.port > 0, "Port must be greater than 0");
        ensure!(
            self.width > 0 && self.height > 0,
            "Resolution must be positive"
        );
        Ok(())
    }

    pub fn generate_command(&self, password: Option<&str>) -> (String, Vec<String>) {
        match self.kind {
            RemoteDesktopKind::Rdp => {
                if cfg!(target_os = "windows") {
                    let mut args = vec![format!("/v:{}:{}", self.host, self.port)];
                    if self.fullscreen {
                        args.push("/f".into());
                    } else {
                        args.push(format!("/w:{}", self.width));
                        args.push(format!("/h:{}", self.height));
                    }
                    ("mstsc.exe".into(), args)
                } else {
                    let mut args = vec![
                        format!("/v:{}:{}", self.host, self.port),
                        format!("/size:{}x{}", self.width, self.height),
                        format!("/bpp:{}", self.color_depth),
                    ];
                    if !self.username.is_empty() {
                        args.push(format!("/u:{}", self.username));
                    }
                    if let Some(dom) = &self.domain {
                        args.push(format!("/d:{}", dom));
                    }
                    if let Some(pwd) = password {
                        args.push(format!("/p:{}", pwd));
                    }
                    if self.fullscreen {
                        args.push("/f".into());
                    }
                    args.extend(self.custom_args.clone());
                    ("xfreerdp".into(), args)
                }
            }
            RemoteDesktopKind::Vnc => {
                let target = format!("{}:{}", self.host, self.port);
                let mut args = vec![target];
                if self.fullscreen {
                    args.push("-fullscreen".into());
                }
                args.extend(self.custom_args.clone());
                ("vncviewer".into(), args)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_defaults() {
        assert_eq!(ProtocolKind::SSH.default_port(), 22);
        assert_eq!(ProtocolKind::Telnet.default_port(), 23);
        assert_eq!(ProtocolKind::FTP.default_port(), 21);
        assert_eq!(ProtocolKind::RDP.default_port(), 3389);
        assert_eq!(ProtocolKind::VNC.default_port(), 5900);
    }

    #[test]
    fn test_serial_config_validation() {
        let mut cfg = SerialConfig::default();
        assert!(cfg.validate().is_ok());
        cfg.data_bits = 4;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_rdp_command_generation() {
        let cfg = RemoteDesktopConfig {
            kind: RemoteDesktopKind::Rdp,
            host: "192.168.1.50".into(),
            port: 3389,
            username: "admin".into(),
            domain: Some("CORP".into()),
            width: 1440,
            height: 900,
            color_depth: 32,
            fullscreen: false,
            custom_args: vec!["/audio-mode:0".into()],
        };
        let (binary, args) = cfg.generate_command(Some("secret123"));
        assert!(!binary.is_empty());
        let joined = args.join(" ");
        assert!(joined.contains("192.168.1.50:3389"));
    }
}
