use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    Hup = 1,
    Int = 2,
    Kill = 9,
    Term = 15,
    Cont = 18,
    Stop = 19,
}

impl Signal {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Hup => "SIGHUP (1)",
            Self::Int => "SIGINT (2)",
            Self::Kill => "SIGKILL (9 - Force Kill)",
            Self::Term => "SIGTERM (15 - Graceful)",
            Self::Cont => "SIGCONT (18)",
            Self::Stop => "SIGSTOP (19)",
        }
    }

    pub fn number(&self) -> u32 {
        *self as u32
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub user: String,
    pub cpu: f32,
    pub mem: f32,
    pub stat: String,
    pub time: String,
    pub command: String,
}

impl ProcessInfo {
    pub fn parse_ps_output(output: &str) -> Vec<ProcessInfo> {
        let mut processes = Vec::new();
        let mut lines = output.lines();
        let first = match lines.next() {
            Some(l) => l,
            None => return processes,
        };

        if !first.to_uppercase().contains("PID")
            && let Some(p) = Self::parse_line(first)
        {
            processes.push(p);
        }

        for line in lines {
            if let Some(p) = Self::parse_line(line) {
                processes.push(p);
            }
        }

        processes
    }

    fn parse_line(line: &str) -> Option<ProcessInfo> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() < 8 {
            return None;
        }

        let pid = parts[0].parse::<u32>().ok()?;
        let ppid = parts[1].parse::<u32>().ok()?;
        let user = parts[2].to_string();
        let cpu = parts[3].parse::<f32>().unwrap_or(0.0);
        let mem = parts[4].parse::<f32>().unwrap_or(0.0);
        let stat = parts[5].to_string();
        let time = parts[6].to_string();
        let command = parts[7..].join(" ");

        Some(ProcessInfo {
            pid,
            ppid,
            user,
            cpu,
            mem,
            stat,
            time,
            command,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ps_output() {
        let output = r#"  PID  PPID USER     %CPU %MEM STAT     TIME COMMAND
    1     0 root      0.0  0.1 Ss   00:00:03 /sbin/init splash
  542     1 root      0.2  1.4 S<sl 00:01:22 /usr/bin/dockerd -H fd://
 1205  1000 edivan    5.4  3.2 R    00:05:43 cargo test --workspace --locked
"#;
        let procs = ProcessInfo::parse_ps_output(output);
        assert_eq!(procs.len(), 3);
        assert_eq!(procs[0].pid, 1);
        assert_eq!(procs[0].ppid, 0);
        assert_eq!(procs[0].user, "root");
        assert_eq!(procs[0].cpu, 0.0);
        assert_eq!(procs[0].mem, 0.1);
        assert_eq!(procs[0].stat, "Ss");
        assert_eq!(procs[0].command, "/sbin/init splash");

        assert_eq!(procs[2].pid, 1205);
        assert_eq!(procs[2].ppid, 1000);
        assert_eq!(procs[2].user, "edivan");
        assert_eq!(procs[2].cpu, 5.4);
        assert_eq!(procs[2].mem, 3.2);
        assert_eq!(procs[2].command, "cargo test --workspace --locked");
    }

    #[test]
    fn test_signals() {
        assert_eq!(Signal::Kill.number(), 9);
        assert_eq!(Signal::Term.number(), 15);
    }
}
