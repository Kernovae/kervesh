use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RecordingFormat {
    #[default]
    AsciicastV2,
    CleanText,
    Raw,
}

impl RecordingFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            RecordingFormat::AsciicastV2 => "cast",
            RecordingFormat::CleanText => "txt",
            RecordingFormat::Raw => "raw",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            RecordingFormat::AsciicastV2 => "Asciicast v2 (.cast)",
            RecordingFormat::CleanText => "Clean Text (.txt)",
            RecordingFormat::Raw => "Raw Stream (.raw)",
        }
    }
}

/// Asciicast v2 file header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsciicastHeader {
    pub version: u8,
    pub width: u16,
    pub height: u16,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<std::collections::HashMap<String, String>>,
}

pub struct SessionRecorder {
    format: RecordingFormat,
    file: Option<File>,
    path: PathBuf,
    start_time: Instant,
    active: bool,
    bytes_written: usize,
}

impl SessionRecorder {
    pub fn default_recordings_dir() -> Result<PathBuf> {
        let dirs = directories::ProjectDirs::from("org", "Kernovae", "Kervesh")
            .context("No local directory")?;
        let path = dirs.data_local_dir().join("recordings");
        std::fs::create_dir_all(&path)?;
        Ok(path)
    }

    pub fn start(
        session_id: &str,
        host_label: &str,
        format: RecordingFormat,
        dir: Option<&Path>,
        cols: u16,
        rows: u16,
    ) -> Result<Self> {
        let recordings_dir = match dir {
            Some(d) => {
                std::fs::create_dir_all(d)?;
                d.to_path_buf()
            }
            None => Self::default_recordings_dir()?,
        };

        let now: DateTime<Utc> = Utc::now();
        let sanitized_label: String = host_label
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let filename = format!(
            "{}_{}_{}.{}",
            sanitized_label,
            now.format("%Y%m%d_%H%M%S"),
            &session_id[..session_id.len().min(8)],
            format.extension()
        );
        let path = recordings_dir.join(filename);

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)?;

        let mut bytes_written = 0;
        if format == RecordingFormat::AsciicastV2 {
            let mut env = std::collections::HashMap::new();
            env.insert("SHELL".to_string(), "/bin/bash".to_string());
            env.insert("TERM".to_string(), "xterm-256color".to_string());

            let header = AsciicastHeader {
                version: 2,
                width: cols.max(10),
                height: rows.max(5),
                timestamp: now.timestamp(),
                title: Some(format!("Kervesh Session - {}", host_label)),
                env: Some(env),
            };
            let mut header_json = serde_json::to_string(&header)?;
            header_json.push('\n');
            file.write_all(header_json.as_bytes())?;
            bytes_written += header_json.len();
        }

        Ok(Self {
            format,
            file: Some(file),
            path,
            start_time: Instant::now(),
            active: true,
            bytes_written,
        })
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn format(&self) -> RecordingFormat {
        self.format
    }

    pub fn bytes_written(&self) -> usize {
        self.bytes_written
    }

    pub fn duration_secs(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }

    pub fn write_output(&mut self, data: &[u8]) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        let Some(file) = &mut self.file else {
            return Ok(());
        };

        match self.format {
            RecordingFormat::Raw => {
                file.write_all(data)?;
                self.bytes_written += data.len();
            }
            RecordingFormat::CleanText => {
                let text = String::from_utf8_lossy(data);
                let cleaned = Self::strip_ansi(&text);
                file.write_all(cleaned.as_bytes())?;
                self.bytes_written += cleaned.len();
            }
            RecordingFormat::AsciicastV2 => {
                let text = String::from_utf8_lossy(data);
                if !text.is_empty() {
                    let elapsed = self.start_time.elapsed().as_secs_f64();
                    let event = serde_json::json!([elapsed, "o", text]);
                    let mut line = serde_json::to_string(&event)?;
                    line.push('\n');
                    file.write_all(line.as_bytes())?;
                    self.bytes_written += line.len();
                }
            }
        }
        file.flush()?;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<PathBuf> {
        self.active = false;
        if let Some(mut file) = self.file.take() {
            file.flush()?;
        }
        Ok(self.path.clone())
    }

    /// Strips ANSI escape codes from string
    pub fn strip_ansi(input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        let mut in_escape = false;
        let mut in_csi = false;
        let mut in_osc = false;

        let mut chars = input.chars().peekable();
        while let Some(c) = chars.next() {
            if in_osc {
                if c == '\x07' {
                    in_osc = false;
                } else if c == '\x1b' && chars.peek() == Some(&'\\') {
                    chars.next();
                    in_osc = false;
                }
                continue;
            }

            if in_csi {
                if ('@'..='~').contains(&c) {
                    in_csi = false;
                }
                continue;
            }

            if in_escape {
                match c {
                    '[' => {
                        in_csi = true;
                        in_escape = false;
                    }
                    ']' => {
                        in_osc = true;
                        in_escape = false;
                    }
                    '(' | ')' | '*' | '+' | '-' | '.' | '/' => {
                        chars.next();
                        in_escape = false;
                    }
                    _ => {
                        in_escape = false;
                    }
                }
                continue;
            }

            if c == '\x1b' {
                in_escape = true;
                continue;
            }

            if c != '\r' {
                out.push(c);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi() {
        let colored = "\x1b[31;1mHello\x1b[0m \x1b[32mWorld\x1b[0m!\r\n";
        let cleaned = SessionRecorder::strip_ansi(colored);
        assert_eq!(cleaned, "Hello World!\n");

        let osc = "\x1b]0;my-title\x07Prompt: ";
        let cleaned_osc = SessionRecorder::strip_ansi(osc);
        assert_eq!(cleaned_osc, "Prompt: ");
    }

    #[test]
    fn test_recorder_asciicast() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut recorder = SessionRecorder::start(
            "sess1234",
            "prod-server",
            RecordingFormat::AsciicastV2,
            Some(temp_dir.path()),
            80,
            24,
        )
        .unwrap();

        assert!(recorder.is_active());
        recorder.write_output(b"echo hello\r\n").unwrap();
        recorder.write_output(b"hello\r\n").unwrap();
        let path = recorder.stop().unwrap();
        assert!(!recorder.is_active());

        let content = std::fs::read_to_string(path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert!(lines.len() >= 3);

        let header: AsciicastHeader = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(header.version, 2);
        assert_eq!(header.width, 80);
        assert_eq!(header.height, 24);
    }

    #[test]
    fn test_recorder_clean_text() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut recorder = SessionRecorder::start(
            "sess5678",
            "dev-box",
            RecordingFormat::CleanText,
            Some(temp_dir.path()),
            80,
            24,
        )
        .unwrap();

        recorder
            .write_output(b"\x1b[34muser@host\x1b[0m:~$ ls\r\n")
            .unwrap();
        let path = recorder.stop().unwrap();

        let content = std::fs::read_to_string(path).unwrap();
        assert_eq!(content, "user@host:~$ ls\n");
    }
}
