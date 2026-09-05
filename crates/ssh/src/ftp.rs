use crate::RemoteEntry;
use anyhow::{Context, Result, bail, ensure};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

pub struct FtpClient {
    host: String,
    port: u16,
    reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: tokio::net::tcp::OwnedWriteHalf,
}

impl FtpClient {
    pub async fn connect(host: &str, port: u16, user: &str, pass: &str) -> Result<Self> {
        let stream = tokio::time::timeout(
            Duration::from_secs(10),
            TcpStream::connect(format!("{}:{}", host, port)),
        )
        .await??;

        let (r, mut writer) = stream.into_split();
        let mut reader = BufReader::new(r);

        // Read initial greeting (220)
        let (code, msg) = Self::read_response(&mut reader).await?;
        ensure!(code == 220, "FTP server greeting failed: {code} {msg}");

        // Send USER
        writer
            .write_all(format!("USER {}\r\n", user).as_bytes())
            .await?;
        let (code, msg) = Self::read_response(&mut reader).await?;
        if code == 331 {
            // Send PASS
            writer
                .write_all(format!("PASS {}\r\n", pass).as_bytes())
                .await?;
            let (code, msg) = Self::read_response(&mut reader).await?;
            ensure!(code == 230, "FTP login failed: {code} {msg}");
        } else {
            ensure!(code == 230, "FTP login failed: {code} {msg}");
        }

        // Set binary transfer mode
        writer.write_all(b"TYPE I\r\n").await?;
        let _ = Self::read_response(&mut reader).await?;

        Ok(Self {
            host: host.to_string(),
            port,
            reader,
            writer,
        })
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    async fn read_response(
        reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>,
    ) -> Result<(u16, String)> {
        let mut line = String::new();
        loop {
            line.clear();
            if reader.read_line(&mut line).await? == 0 {
                bail!("Connection closed by FTP server");
            }
            if line.len() >= 3 && line.chars().take(3).all(|c| c.is_ascii_digit()) {
                let code: u16 = line[..3].parse().unwrap_or(0);
                if line.len() == 3 || line.as_bytes()[3] == b' ' {
                    return Ok((code, line[3..].trim().to_string()));
                }
            }
        }
    }

    pub async fn send_cmd(&mut self, cmd: &str) -> Result<(u16, String)> {
        self.writer
            .write_all(format!("{}\r\n", cmd).as_bytes())
            .await?;
        Self::read_response(&mut self.reader).await
    }

    pub async fn pwd(&mut self) -> Result<String> {
        let (code, msg) = self.send_cmd("PWD").await?;
        ensure!(code == 257, "PWD failed: {code} {msg}");
        if let Some(start) = msg.find('"')
            && let Some(end) = msg[start + 1..].find('"')
        {
            return Ok(msg[start + 1..start + 1 + end].to_string());
        }
        Ok(msg)
    }

    pub async fn cwd(&mut self, path: &str) -> Result<()> {
        let (code, msg) = self.send_cmd(&format!("CWD {}", path)).await?;
        ensure!(code == 250, "CWD failed: {code} {msg}");
        Ok(())
    }

    pub async fn pasv_connect(&mut self) -> Result<TcpStream> {
        let (code, msg) = self.send_cmd("PASV").await?;
        ensure!(code == 227, "PASV failed: {code} {msg}");

        let (data_ip, data_port) = Self::parse_pasv_response(&msg)?;
        let ip = if data_ip == "0.0.0.0" || data_ip.starts_with("127.") {
            self.host.clone()
        } else {
            data_ip
        };

        tokio::time::timeout(
            Duration::from_secs(10),
            TcpStream::connect(format!("{}:{}", ip, data_port)),
        )
        .await?
        .map_err(Into::into)
    }

    pub fn parse_pasv_response(msg: &str) -> Result<(String, u16)> {
        let start = msg
            .find('(')
            .or_else(|| msg.find(','))
            .context("Invalid PASV format")?;
        let end = msg.rfind(')').unwrap_or(msg.len());
        let slice = &msg[start..end].trim_matches(|c| c == '(' || c == ')');
        let parts: Vec<u8> = slice
            .split(',')
            .filter_map(|s| s.trim().parse::<u8>().ok())
            .collect();
        ensure!(parts.len() >= 6, "Malformed PASV response");
        let ip = format!("{}.{}.{}.{}", parts[0], parts[1], parts[2], parts[3]);
        let port = ((parts[4] as u16) << 8) | (parts[5] as u16);
        Ok((ip, port))
    }

    pub async fn list(&mut self, path: &str) -> Result<Vec<RemoteEntry>> {
        let mut data_stream = self.pasv_connect().await?;
        let cmd = if path.is_empty() {
            "LIST".into()
        } else {
            format!("LIST {}", path)
        };
        let (code, msg) = self.send_cmd(&cmd).await?;
        ensure!(code == 150 || code == 125, "LIST failed: {code} {msg}");

        let mut raw_data = Vec::new();
        data_stream.read_to_end(&mut raw_data).await?;
        drop(data_stream);

        let (code, _) = Self::read_response(&mut self.reader).await?;
        ensure!(code == 226 || code == 250, "LIST transfer incomplete");

        let text = String::from_utf8_lossy(&raw_data);
        Ok(Self::parse_list_output(&text))
    }

    pub fn parse_list_output(output: &str) -> Vec<RemoteEntry> {
        let mut entries = Vec::new();
        for line in output.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 9 {
                continue;
            }
            let is_dir = parts[0].starts_with('d');
            let is_symlink = parts[0].starts_with('l');
            let size = parts[4].parse::<u64>().unwrap_or(0);
            let name = parts[8..].join(" ");
            if name == "." || name == ".." {
                continue;
            }

            entries.push(RemoteEntry {
                name,
                directory: is_dir,
                symlink: is_symlink,
                size,
                permissions: None,
                uid: None,
                gid: None,
                modified: None,
            });
        }
        entries
    }

    pub async fn delete(&mut self, path: &str) -> Result<()> {
        let (code, msg) = self.send_cmd(&format!("DELE {}", path)).await?;
        ensure!(code == 250, "DELE failed: {code} {msg}");
        Ok(())
    }

    pub async fn mkdir(&mut self, path: &str) -> Result<()> {
        let (code, msg) = self.send_cmd(&format!("MKD {}", path)).await?;
        ensure!(code == 257, "MKD failed: {code} {msg}");
        Ok(())
    }

    pub async fn rmdir(&mut self, path: &str) -> Result<()> {
        let (code, msg) = self.send_cmd(&format!("RMD {}", path)).await?;
        ensure!(code == 250, "RMD failed: {code} {msg}");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pasv_parsing() {
        let resp = "227 Entering Passive Mode (192,168,1,100,195,80).";
        let (ip, port) = FtpClient::parse_pasv_response(resp).unwrap();
        assert_eq!(ip, "192.168.1.100");
        assert_eq!(port, (195 << 8) | 80);
    }

    #[test]
    fn test_list_parsing() {
        let sample = "drwxr-xr-x 2 user group 4096 Sep 05 12:00 public_html\r\n-rw-r--r-- 1 user group 1024 Sep 05 12:01 index.html\r\n";
        let entries = FtpClient::parse_list_output(sample);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "public_html");
        assert!(entries[0].directory);
        assert_eq!(entries[1].name, "index.html");
        assert!(!entries[1].directory);
        assert_eq!(entries[1].size, 1024);
    }
}
