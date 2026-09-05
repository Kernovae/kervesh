use anyhow::Result;
use kervesh_core::TelnetConfig;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

// Telnet Command Constants (RFC 854)
pub const SE: u8 = 240;
pub const NOP: u8 = 241;
pub const DM: u8 = 242;
pub const BRK: u8 = 243;
pub const IP: u8 = 244;
pub const AO: u8 = 245;
pub const AYT: u8 = 246;
pub const EC: u8 = 247;
pub const EL: u8 = 248;
pub const GA: u8 = 249;
pub const SB: u8 = 250;
pub const WILL: u8 = 251;
pub const WONT: u8 = 252;
pub const DO: u8 = 253;
pub const DONT: u8 = 254;
pub const IAC: u8 = 255;

// Telnet Options (RFC 855 / RFC 1091 / RFC 1073)
pub const OPT_BINARY: u8 = 0;
pub const OPT_ECHO: u8 = 1;
pub const OPT_SGA: u8 = 3;
pub const OPT_TTYPE: u8 = 24;
pub const OPT_NAWS: u8 = 31;
pub const OPT_LINEMODE: u8 = 34;

pub struct TelnetParser {
    terminal_type: String,
    cols: u16,
    rows: u16,
    in_subneg: bool,
    subneg_buf: Vec<u8>,
}

impl TelnetParser {
    pub fn new(terminal_type: String, cols: u16, rows: u16) -> Self {
        Self {
            terminal_type,
            cols,
            rows,
            in_subneg: false,
            subneg_buf: Vec::new(),
        }
    }

    pub fn set_window_size(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
    }

    pub fn build_naws_subneg(&self) -> Vec<u8> {
        let (w_hi, w_lo) = ((self.cols >> 8) as u8, (self.cols & 0xFF) as u8);
        let (h_hi, h_lo) = ((self.rows >> 8) as u8, (self.rows & 0xFF) as u8);
        vec![IAC, SB, OPT_NAWS, w_hi, w_lo, h_hi, h_lo, IAC, SE]
    }

    /// Parses incoming raw bytes from Telnet stream.
    /// Returns (clean_terminal_output, responses_to_send).
    pub fn process_incoming(&mut self, input: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let mut clean_output = Vec::new();
        let mut replies = Vec::new();
        let mut i = 0;

        while i < input.len() {
            let b = input[i];

            if self.in_subneg {
                if b == IAC && i + 1 < input.len() && input[i + 1] == SE {
                    self.in_subneg = false;
                    self.handle_subneg(&mut replies);
                    self.subneg_buf.clear();
                    i += 2;
                    continue;
                } else {
                    self.subneg_buf.push(b);
                    i += 1;
                    continue;
                }
            }

            if b == IAC {
                if i + 1 >= input.len() {
                    break;
                }
                let cmd = input[i + 1];

                if cmd == IAC {
                    // Escaped 255 literal
                    clean_output.push(255);
                    i += 2;
                    continue;
                }

                if cmd == SB {
                    self.in_subneg = true;
                    self.subneg_buf.clear();
                    i += 2;
                    continue;
                }

                if cmd == DO || cmd == DONT || cmd == WILL || cmd == WONT {
                    if i + 2 >= input.len() {
                        break;
                    }
                    let opt = input[i + 2];
                    self.handle_negotiation(cmd, opt, &mut replies);
                    i += 3;
                    continue;
                }

                // Other simple IAC commands (NOP, GA, etc.)
                i += 2;
                continue;
            }

            clean_output.push(b);
            i += 1;
        }

        (clean_output, replies)
    }

    fn handle_negotiation(&mut self, cmd: u8, opt: u8, replies: &mut Vec<u8>) {
        match cmd {
            DO => match opt {
                OPT_NAWS => {
                    replies.extend_from_slice(&[IAC, WILL, OPT_NAWS]);
                    replies.extend_from_slice(&self.build_naws_subneg());
                }
                OPT_TTYPE => {
                    replies.extend_from_slice(&[IAC, WILL, OPT_TTYPE]);
                }
                OPT_SGA => {
                    replies.extend_from_slice(&[IAC, WILL, OPT_SGA]);
                }
                OPT_BINARY => {
                    replies.extend_from_slice(&[IAC, WILL, OPT_BINARY]);
                }
                _ => {
                    replies.extend_from_slice(&[IAC, WONT, opt]);
                }
            },
            DONT => {
                replies.extend_from_slice(&[IAC, WONT, opt]);
            }
            WILL => match opt {
                OPT_ECHO => {
                    replies.extend_from_slice(&[IAC, DO, OPT_ECHO]);
                }
                OPT_SGA => {
                    replies.extend_from_slice(&[IAC, DO, OPT_SGA]);
                }
                OPT_BINARY => {
                    replies.extend_from_slice(&[IAC, DO, OPT_BINARY]);
                }
                _ => {
                    replies.extend_from_slice(&[IAC, DONT, opt]);
                }
            },
            WONT => {
                replies.extend_from_slice(&[IAC, DONT, opt]);
            }
            _ => {}
        }
    }

    fn handle_subneg(&mut self, replies: &mut Vec<u8>) {
        if self.subneg_buf.is_empty() {
            return;
        }

        let opt = self.subneg_buf[0];
        if opt == OPT_TTYPE && self.subneg_buf.len() >= 2 && self.subneg_buf[1] == 1 {
            // SEND terminal type requested: respond with [IAC, SB, TTYPE, 0, <ttype>, IAC, SE]
            replies.extend_from_slice(&[IAC, SB, OPT_TTYPE, 0]);
            replies.extend_from_slice(self.terminal_type.as_bytes());
            replies.extend_from_slice(&[IAC, SE]);
        }
    }

    /// Escapes client input for sending over Telnet stream (escapes 255 as [255, 255]).
    pub fn escape_client_input(input: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(input.len());
        for &b in input {
            if b == IAC {
                out.push(IAC);
                out.push(IAC);
            } else {
                out.push(b);
            }
        }
        out
    }
}

pub async fn run_telnet_session(
    config: TelnetConfig,
    mut input_rx: mpsc::Receiver<Vec<u8>>,
    output_tx: mpsc::Sender<Vec<u8>>,
    mut resize_rx: mpsc::Receiver<(u16, u16)>,
) -> Result<()> {
    let addr = format!("{}:{}", config.host, config.port);
    let mut stream =
        tokio::time::timeout(Duration::from_secs(10), TcpStream::connect(&addr)).await??;

    let mut parser = TelnetParser::new(config.terminal_type, 80, 24);

    let (mut reader, mut writer) = stream.split();
    let mut buf = [0u8; 4096];

    loop {
        tokio::select! {
            res = reader.read(&mut buf) => {
                match res {
                    Ok(0) => break,
                    Ok(n) => {
                        let (clean, replies) = parser.process_incoming(&buf[..n]);
                        if !replies.is_empty() {
                            writer.write_all(&replies).await?;
                        }
                        if !clean.is_empty() {
                            let _ = output_tx.send(clean).await;
                        }
                    }
                    Err(_) => break,
                }
            }
            Some(input) = input_rx.recv() => {
                let escaped = TelnetParser::escape_client_input(&input);
                if writer.write_all(&escaped).await.is_err() {
                    break;
                }
            }
            Some((cols, rows)) = resize_rx.recv() => {
                parser.set_window_size(cols, rows);
                if config.naws {
                    let naws_subneg = parser.build_naws_subneg();
                    let _ = writer.write_all(&naws_subneg).await;
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telnet_input_escaping() {
        let input = vec![b'h', b'e', b'l', b'l', b'o', 255, b'x'];
        let escaped = TelnetParser::escape_client_input(&input);
        assert_eq!(escaped, vec![b'h', b'e', b'l', b'l', b'o', 255, 255, b'x']);
    }

    #[test]
    fn test_telnet_negotiation_response() {
        let mut parser = TelnetParser::new("xterm".into(), 100, 30);
        // Server sends DO NAWS: [255, 253, 31]
        let (output, replies) = parser.process_incoming(&[IAC, DO, OPT_NAWS]);
        assert!(output.is_empty());
        // Should respond with WILL NAWS and NAWS subnegotiation
        assert!(replies.contains(&IAC));
        assert!(replies.contains(&WILL));
        assert!(replies.contains(&OPT_NAWS));
        assert!(replies.contains(&SB));
    }

    #[test]
    fn test_telnet_clean_data_passthrough() {
        let mut parser = TelnetParser::new("xterm".into(), 80, 24);
        let raw = b"Welcome to Linux!\r\n";
        let (output, replies) = parser.process_incoming(raw);
        assert_eq!(output, raw);
        assert!(replies.is_empty());
    }
}
