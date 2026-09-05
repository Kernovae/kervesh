use anyhow::{Result, ensure};
use std::collections::BTreeMap;

pub const PROBE: &str = "uname -s; uname -r; uname -m; hostname; test -r /proc/stat && printf 'PROCFS\\n'; command -v df >/dev/null && printf 'DF\\n'";
pub const COLLECT: &str = "export LC_ALL=C; printf '@@stat\\n'; cat /proc/stat; printf '@@mem\\n'; cat /proc/meminfo; printf '@@net\\n'; cat /proc/net/dev; printf '@@load\\n'; cat /proc/loadavg; printf '@@uptime\\n'; cat /proc/uptime; printf '@@df\\n'; df -Pk; printf '@@inodes\\n'; df -Pi; printf '@@fd\\n'; cat /proc/sys/fs/file-nr; printf '@@cpuinfo\\n'; head -80 /proc/cpuinfo";

#[derive(Clone, Debug, Default)]
pub struct RemoteCapabilities {
    pub os: String,
    pub kernel: String,
    pub architecture: String,
    pub hostname: String,
    pub procfs: bool,
    pub df: bool,
}
impl RemoteCapabilities {
    pub fn parse(text: &str) -> Self {
        let lines: Vec<_> = text.lines().collect();
        Self {
            os: lines.first().unwrap_or(&"Unknown").to_string(),
            kernel: lines.get(1).unwrap_or(&"").to_string(),
            architecture: lines.get(2).unwrap_or(&"").to_string(),
            hostname: lines.get(3).unwrap_or(&"").to_string(),
            procfs: lines.contains(&"PROCFS"),
            df: lines.contains(&"DF"),
        }
    }
}
#[derive(Clone, Debug)]
pub struct Filesystem {
    pub device: String,
    pub mount: String,
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub percent: f32,
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub cpu: BTreeMap<String, (u64, u64)>,
    pub memory: BTreeMap<String, u64>,
    pub network: BTreeMap<String, (u64, u64)>,
    pub load: Option<[f64; 3]>,
    pub uptime: Option<f64>,
    pub filesystems: Vec<Filesystem>,
    pub inodes: String,
    pub file_descriptors: String,
    pub cpu_model: String,
    pub processes: Option<u64>,
}
#[derive(Clone, Debug, Default)]
pub struct Rates {
    pub cpu: Option<f64>,
    pub cores: BTreeMap<String, f64>,
    pub network: BTreeMap<String, (f64, f64)>,
}
impl Snapshot {
    pub fn parse(text: &str) -> Result<Self> {
        let mut value = Self::default();
        let mut section = "";
        let mut recognized = false;
        for line in text.lines() {
            if let Some(s) = line.strip_prefix("@@") {
                section = s;
                continue;
            }
            let fields: Vec<&str> = line.split_whitespace().collect();
            match section {
                "stat"
                    if fields.first().is_some_and(|s| s.starts_with("cpu"))
                        && fields.len() >= 5 =>
                {
                    let nums: Vec<u64> = fields[1..]
                        .iter()
                        .take(8)
                        .map(|s| s.parse().unwrap_or(0))
                        .collect();
                    value.cpu.insert(
                        fields[0].into(),
                        (
                            nums.iter().sum(),
                            nums[3] + nums.get(4).copied().unwrap_or(0),
                        ),
                    );
                    recognized = true;
                }
                "mem" if fields.len() >= 2 => {
                    if let Ok(n) = fields[1].parse::<u64>() {
                        value.memory.insert(
                            fields[0].trim_end_matches(':').into(),
                            n.saturating_mul(1024),
                        );
                        recognized = true;
                    }
                }
                "net" => {
                    if let Some((name, counters)) = line.split_once(':') {
                        let nums: Vec<_> = counters.split_whitespace().collect();
                        if nums.len() >= 9
                            && let (Ok(rx), Ok(tx)) = (nums[0].parse(), nums[8].parse())
                        {
                            value.network.insert(name.trim().into(), (rx, tx));
                            recognized = true;
                        }
                    }
                }
                "load" if fields.len() >= 3 => {
                    if let (Ok(a), Ok(b), Ok(c)) =
                        (fields[0].parse(), fields[1].parse(), fields[2].parse())
                    {
                        value.load = Some([a, b, c]);
                    }
                    value.processes = fields
                        .get(3)
                        .and_then(|s| s.split_once('/'))
                        .and_then(|(_, n)| n.parse().ok());
                }
                "uptime" => value.uptime = fields.first().and_then(|s| s.parse().ok()),
                "df" if fields.len() >= 6 => {
                    if let (Ok(total), Ok(used), Ok(available), Ok(percent)) = (
                        fields[1].parse::<u64>(),
                        fields[2].parse::<u64>(),
                        fields[3].parse::<u64>(),
                        fields[4].trim_end_matches('%').parse::<f32>(),
                    ) {
                        value.filesystems.push(Filesystem {
                            device: fields[0].into(),
                            mount: fields[5..].join(" "),
                            total: total.saturating_mul(1024),
                            used: used.saturating_mul(1024),
                            available: available.saturating_mul(1024),
                            percent,
                        });
                    }
                }
                "inodes" => {
                    value.inodes.push_str(line);
                    value.inodes.push('\n');
                }
                "fd" => value.file_descriptors = line.into(),
                "cpuinfo" if line.starts_with("model name") || line.starts_with("Hardware") => {
                    value.cpu_model = line
                        .split_once(':')
                        .map(|(_, v)| v.trim().into())
                        .unwrap_or_default();
                }
                _ => {}
            }
        }
        ensure!(
            recognized,
            "Remote metrics unavailable: no readable procfs counters"
        );
        Ok(value)
    }
    pub fn memory_used(&self) -> Option<u64> {
        let total = *self.memory.get("MemTotal")?;
        let available = self.memory.get("MemAvailable").copied().unwrap_or_else(|| {
            ["MemFree", "Buffers", "Cached"]
                .iter()
                .map(|k| self.memory.get(*k).copied().unwrap_or(0))
                .sum()
        });
        Some(total.saturating_sub(available))
    }
    pub fn swap_used(&self) -> Option<u64> {
        Some(
            self.memory
                .get("SwapTotal")?
                .saturating_sub(*self.memory.get("SwapFree")?),
        )
    }
    pub fn rates(&self, previous: &Self, elapsed: f64) -> Rates {
        let mut rates = Rates::default();
        if !elapsed.is_finite() || elapsed <= 0.0 {
            return rates;
        }
        for (name, (total, idle)) in &self.cpu {
            if let Some((pt, pi)) = previous.cpu.get(name) {
                let dt = total.saturating_sub(*pt);
                let di = idle.saturating_sub(*pi);
                if dt > 0 {
                    let percent = 100.0 * dt.saturating_sub(di) as f64 / dt as f64;
                    if name == "cpu" {
                        rates.cpu = Some(percent);
                    } else {
                        rates.cores.insert(name.clone(), percent);
                    }
                }
            }
        }
        for (name, (rx, tx)) in &self.network {
            if let Some((pr, pt)) = previous.network.get(name) {
                rates.network.insert(
                    name.clone(),
                    (
                        rx.saturating_sub(*pr) as f64 / elapsed,
                        tx.saturating_sub(*pt) as f64 / elapsed,
                    ),
                );
            }
        }
        rates
    }
}
pub fn bytes(value: u64) -> String {
    let units = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut amount = value as f64;
    let mut index = 0;
    while amount >= 1024.0 && index < units.len() - 1 {
        amount /= 1024.0;
        index += 1;
    }
    format!("{amount:.1} {}", units[index])
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct MonitorThresholds {
    pub cpu_percent: f32,
    pub memory_percent: f32,
    pub swap_percent: f32,
    pub disk_percent: f32,
    pub inode_percent: f32,
    pub load_per_core: f32,
}

impl Default for MonitorThresholds {
    fn default() -> Self {
        Self {
            cpu_percent: 85.0,
            memory_percent: 90.0,
            swap_percent: 80.0,
            disk_percent: 90.0,
            inode_percent: 90.0,
            load_per_core: 2.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlertSeverity {
    Warning,
    Critical,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MonitorAlert {
    pub metric: String,
    pub message: String,
    pub severity: AlertSeverity,
    pub value: f32,
    pub threshold: f32,
}

impl Snapshot {
    pub fn check_alerts(&self, rates: &Rates, thresholds: &MonitorThresholds) -> Vec<MonitorAlert> {
        let mut alerts = Vec::new();

        if let Some(cpu) = rates.cpu {
            let cpu_f32 = cpu as f32;
            if cpu_f32 >= thresholds.cpu_percent {
                alerts.push(MonitorAlert {
                    metric: "CPU".into(),
                    message: format!("CPU usage is high ({:.1}%)", cpu_f32),
                    severity: if cpu_f32 >= 95.0 {
                        AlertSeverity::Critical
                    } else {
                        AlertSeverity::Warning
                    },
                    value: cpu_f32,
                    threshold: thresholds.cpu_percent,
                });
            }
        }

        if let (Some(used), Some(total)) = (self.memory_used(), self.memory.get("MemTotal"))
            && *total > 0
        {
            let mem_pct = (used as f32 / *total as f32) * 100.0;
            if mem_pct >= thresholds.memory_percent {
                alerts.push(MonitorAlert {
                    metric: "Memory".into(),
                    message: format!("RAM usage is high ({:.1}%)", mem_pct),
                    severity: if mem_pct >= 95.0 {
                        AlertSeverity::Critical
                    } else {
                        AlertSeverity::Warning
                    },
                    value: mem_pct,
                    threshold: thresholds.memory_percent,
                });
            }
        }

        if let (Some(used), Some(total)) = (self.swap_used(), self.memory.get("SwapTotal"))
            && *total > 0
        {
            let swap_pct = (used as f32 / *total as f32) * 100.0;
            if swap_pct >= thresholds.swap_percent {
                alerts.push(MonitorAlert {
                    metric: "Swap".into(),
                    message: format!("Swap usage is high ({:.1}%)", swap_pct),
                    severity: if swap_pct >= 90.0 {
                        AlertSeverity::Critical
                    } else {
                        AlertSeverity::Warning
                    },
                    value: swap_pct,
                    threshold: thresholds.swap_percent,
                });
            }
        }

        for fs in &self.filesystems {
            if fs.percent >= thresholds.disk_percent {
                alerts.push(MonitorAlert {
                    metric: format!("Disk ({})", fs.mount),
                    message: format!("Filesystem {} is at {:.0}%", fs.mount, fs.percent),
                    severity: if fs.percent >= 95.0 {
                        AlertSeverity::Critical
                    } else {
                        AlertSeverity::Warning
                    },
                    value: fs.percent,
                    threshold: thresholds.disk_percent,
                });
            }
        }

        if let Some(load) = self.load {
            let core_count = self
                .cpu
                .keys()
                .filter(|k| k.starts_with("cpu") && *k != "cpu")
                .count()
                .max(1) as f32;
            let load_1m = load[0] as f32;
            let load_ratio = load_1m / core_count;
            if load_ratio >= thresholds.load_per_core {
                alerts.push(MonitorAlert {
                    metric: "Load".into(),
                    message: format!(
                        "1-min load average ({:.2}) exceeds threshold ({:.1}x cores)",
                        load_1m, load_ratio
                    ),
                    severity: if load_ratio >= thresholds.load_per_core * 1.5 {
                        AlertSeverity::Critical
                    } else {
                        AlertSeverity::Warning
                    },
                    value: load_ratio,
                    threshold: thresholds.load_per_core,
                });
            }
        }

        alerts
    }
}
