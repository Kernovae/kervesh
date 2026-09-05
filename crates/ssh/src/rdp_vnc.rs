use anyhow::Result;
use kervesh_core::RemoteDesktopConfig;
use std::process::Stdio;
use tokio::process::Command;

#[derive(Clone, Debug)]
pub struct RemoteDesktopLauncher;

impl RemoteDesktopLauncher {
    pub fn is_client_available(binary: &str) -> bool {
        std::process::Command::new(binary)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
    }

    pub fn find_available_client(config: &RemoteDesktopConfig) -> Option<String> {
        match config.kind {
            kervesh_core::RemoteDesktopKind::Rdp => {
                if cfg!(windows) {
                    Some("mstsc.exe".into())
                } else {
                    for bin in &["xfreerdp", "wlfreerdp", "remmina"] {
                        if Self::is_client_available(bin) {
                            return Some(bin.to_string());
                        }
                    }
                    Some("xfreerdp".into())
                }
            }
            kervesh_core::RemoteDesktopKind::Vnc => {
                for bin in &["vncviewer", "tigervnc", "remmina"] {
                    if Self::is_client_available(bin) {
                        return Some(bin.to_string());
                    }
                }
                Some("vncviewer".into())
            }
        }
    }

    pub async fn launch(config: &RemoteDesktopConfig, password: Option<&str>) -> Result<u32> {
        config.validate()?;
        let (binary, args) = config.generate_command(password);

        let mut child = Command::new(&binary)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                anyhow::anyhow!("Failed to spawn remote desktop client '{}': {e}", binary)
            })?;

        let pid = child.id().unwrap_or(0);
        tokio::spawn(async move {
            let _ = child.wait().await;
        });

        Ok(pid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_detection() {
        let cfg = RemoteDesktopConfig::default();
        let client = RemoteDesktopLauncher::find_available_client(&cfg);
        assert!(client.is_some());
    }
}
