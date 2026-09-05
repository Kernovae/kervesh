use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Docker / Containers
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerContainer {
    pub id: String,
    pub image: String,
    pub command: String,
    pub created: String,
    pub status: String,
    pub ports: String,
    pub names: String,
    pub running: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImage {
    pub repository: String,
    pub tag: String,
    pub image_id: String,
    pub created: String,
    pub size: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DockerAction {
    Start,
    Stop,
    Restart,
    Pause,
    Unpause,
    Remove,
}

impl DockerAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::Pause => "pause",
            Self::Unpause => "unpause",
            Self::Remove => "rm",
        }
    }
}

impl DockerContainer {
    pub fn parse_json_lines(output: &str) -> Vec<Self> {
        let mut containers = Vec::new();
        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                let id = val["ID"].as_str().unwrap_or_default().to_string();
                let image = val["Image"].as_str().unwrap_or_default().to_string();
                let command = val["Command"].as_str().unwrap_or_default().to_string();
                let created = val["CreatedAt"]
                    .as_str()
                    .or_else(|| val["RunningFor"].as_str())
                    .unwrap_or_default()
                    .to_string();
                let status = val["Status"].as_str().unwrap_or_default().to_string();
                let ports = val["Ports"].as_str().unwrap_or_default().to_string();
                let names = val["Names"].as_str().unwrap_or_default().to_string();
                let state = val["State"].as_str().unwrap_or_default().to_lowercase();
                let running = state.contains("running") || status.to_lowercase().starts_with("up");

                if !id.is_empty() || !names.is_empty() {
                    containers.push(Self {
                        id,
                        image,
                        command,
                        created,
                        status,
                        ports,
                        names,
                        running,
                    });
                }
            }
        }
        containers
    }
}

impl DockerImage {
    pub fn parse_json_lines(output: &str) -> Vec<Self> {
        let mut images = Vec::new();
        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                let repository = val["Repository"].as_str().unwrap_or_default().to_string();
                let tag = val["Tag"].as_str().unwrap_or_default().to_string();
                let image_id = val["ID"].as_str().unwrap_or_default().to_string();
                let created = val["CreatedAt"]
                    .as_str()
                    .or_else(|| val["CreatedSince"].as_str())
                    .unwrap_or_default()
                    .to_string();
                let size = val["Size"].as_str().unwrap_or_default().to_string();

                if !image_id.is_empty() || !repository.is_empty() {
                    images.push(Self {
                        repository,
                        tag,
                        image_id,
                        created,
                        size,
                    });
                }
            }
        }
        images
    }
}

// ---------------------------------------------------------------------------
// Systemd Units
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemdUnit {
    pub name: String,
    pub load: String,
    pub active: String,
    pub sub: String,
    pub description: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemdAction {
    Start,
    Stop,
    Restart,
    Enable,
    Disable,
}

impl SystemdAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::Enable => "enable",
            Self::Disable => "disable",
        }
    }
}

impl SystemdUnit {
    pub fn parse_list_units(output: &str) -> Vec<Self> {
        let mut units = Vec::new();
        for line in output.lines() {
            let line = line.trim();
            if line.is_empty()
                || line.starts_with("UNIT")
                || line.starts_with("LOAD")
                || line.contains("loaded units listed")
            {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 {
                let name = parts[0].to_string();
                let load = parts[1].to_string();
                let active = parts[2].to_string();
                let sub = parts[3].to_string();
                let description = parts[4..].join(" ");

                units.push(Self {
                    name,
                    load,
                    active,
                    sub,
                    description,
                });
            }
        }
        units
    }

    pub fn is_active(&self) -> bool {
        self.active.to_lowercase() == "active"
    }

    pub fn is_failed(&self) -> bool {
        self.active.to_lowercase() == "failed" || self.sub.to_lowercase() == "failed"
    }
}

// ---------------------------------------------------------------------------
// Network Diagnostics Hub
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetDiagTool {
    #[default]
    Ping,
    Traceroute,
    PortScan,
    DnsLookup,
}

impl NetDiagTool {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ping => "Ping (ICMP)",
            Self::Traceroute => "Traceroute (MTR / Hops)",
            Self::PortScan => "Port Scan (NC / Nmap)",
            Self::DnsLookup => "DNS Lookup (Dig / Host)",
        }
    }

    pub fn build_command(&self, target: &str, port_or_type: Option<&str>) -> String {
        let safe_target = target.replace('\'', "");
        match self {
            Self::Ping => format!("ping -c 4 '{}' 2>&1", safe_target),
            Self::Traceroute => format!(
                "traceroute -m 20 '{}' 2>&1 || tracepath -m 20 '{}' 2>&1",
                safe_target, safe_target
            ),
            Self::PortScan => {
                let ports = port_or_type.unwrap_or("22,80,443,3306,5432,6379,8080");
                format!(
                    "for p in $(echo '{}' | tr ',' ' '); do nc -z -v -w 2 '{}' \"$p\" 2>&1; done",
                    ports, safe_target
                )
            }
            Self::DnsLookup => {
                let qtype = port_or_type.unwrap_or("A");
                format!(
                    "dig +short -t {} '{}' 2>&1 || host -t {} '{}' 2>&1",
                    qtype, safe_target, qtype, safe_target
                )
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetDiagResult {
    pub tool: NetDiagTool,
    pub target: String,
    pub raw_output: String,
    pub success: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docker_json_parsing() {
        let sample = r#"{"ID":"a1b2c3d4e5f6","Image":"nginx:latest","Command":"nginx -g 'daemon off;'","CreatedAt":"2 hours ago","Status":"Up 2 hours","Ports":"0.0.0.0:80->80/tcp","Names":"web-server","State":"running"}"#;
        let containers = DockerContainer::parse_json_lines(sample);
        assert_eq!(containers.len(), 1);
        assert_eq!(containers[0].id, "a1b2c3d4e5f6");
        assert_eq!(containers[0].names, "web-server");
        assert!(containers[0].running);
    }

    #[test]
    fn test_systemd_unit_parsing() {
        let sample = "nginx.service           loaded active running A high performance web server and a reverse proxy server\nssh.service             loaded active running OpenBSD Secure Shell server\n";
        let units = SystemdUnit::parse_list_units(sample);
        assert_eq!(units.len(), 2);
        assert_eq!(units[0].name, "nginx.service");
        assert!(units[0].is_active());
        assert_eq!(units[1].name, "ssh.service");
    }

    #[test]
    fn test_netdiag_command_generation() {
        let ping_cmd = NetDiagTool::Ping.build_command("example.com", None);
        assert!(ping_cmd.contains("ping -c 4 'example.com'"));

        let dns_cmd = NetDiagTool::DnsLookup.build_command("example.com", Some("MX"));
        assert!(dns_cmd.contains("dig +short -t MX 'example.com'"));
    }
}
