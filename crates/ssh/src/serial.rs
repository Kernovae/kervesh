use anyhow::Result;
use kervesh_core::SerialConfig;
#[cfg(unix)]
use std::path::Path;
#[cfg(unix)]
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredSerialPort {
    pub port_name: String,
    pub description: Option<String>,
}

pub fn list_available_serial_ports() -> Vec<DiscoveredSerialPort> {
    let mut ports = Vec::new();

    #[cfg(unix)]
    {
        // Check /dev/serial/by-id
        if let Ok(entries) = std::fs::read_dir("/dev/serial/by-id") {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Ok(target) = std::fs::canonicalize(&p) {
                    ports.push(DiscoveredSerialPort {
                        port_name: target.to_string_lossy().to_string(),
                        description: Some(
                            p.file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string(),
                        ),
                    });
                }
            }
        }

        // Scan standard tty devices
        for prefix in &["/dev/ttyUSB", "/dev/ttyACM", "/dev/ttyS"] {
            for i in 0..16 {
                let dev_path = format!("{}{}", prefix, i);
                if Path::new(&dev_path).exists() && !ports.iter().any(|p| p.port_name == dev_path) {
                    ports.push(DiscoveredSerialPort {
                        port_name: dev_path,
                        description: None,
                    });
                }
            }
        }
    }

    #[cfg(windows)]
    {
        for i in 1..=32 {
            let name = format!("COM{}", i);
            ports.push(DiscoveredSerialPort {
                port_name: name,
                description: None,
            });
        }
    }

    if ports.is_empty() {
        // Fallback default
        ports.push(DiscoveredSerialPort {
            port_name: if cfg!(windows) {
                "COM1".into()
            } else {
                "/dev/ttyUSB0".into()
            },
            description: Some("Default Serial Port".into()),
        });
    }

    ports
}

pub async fn run_serial_session(
    config: SerialConfig,
    #[allow(unused_mut)] mut input_rx: mpsc::Receiver<Vec<u8>>,
    output_tx: mpsc::Sender<Vec<u8>>,
) -> Result<()> {
    config.validate()?;

    #[cfg(unix)]
    {
        // Configure baud rate via stty
        let _ = std::process::Command::new("stty")
            .arg("-F")
            .arg(&config.port)
            .arg(config.baud_rate.to_string())
            .arg("raw")
            .arg("-echo")
            .status();

        let file = tokio::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&config.port)
            .await?;

        let (mut reader, mut writer) = tokio::io::split(file);
        let mut buf = [0u8; 2048];

        loop {
            tokio::select! {
                res = reader.read(&mut buf) => {
                    match res {
                        Ok(0) => break,
                        Ok(n) => {
                            let _ = output_tx.send(buf[..n].to_vec()).await;
                        }
                        Err(_) => break,
                    }
                }
                Some(input) = input_rx.recv() => {
                    if writer.write_all(&input).await.is_err() {
                        break;
                    }
                }
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = (config, input_rx, output_tx);
        anyhow::bail!("Serial port direct access is not available on this platform");
    }

    #[allow(unreachable_code)]
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_serial_ports() {
        let ports = list_available_serial_ports();
        assert!(!ports.is_empty());
    }
}
