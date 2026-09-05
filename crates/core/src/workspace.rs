use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionWorkspace {
    pub id: String,
    pub name: String,
    pub description: String,
    pub host_ids: Vec<String>,
    pub tags: Vec<String>,
    pub auto_reconnect_all: bool,
}

impl SessionWorkspace {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            description: description.into(),
            host_ids: Vec::new(),
            tags: Vec::new(),
            auto_reconnect_all: false,
        }
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.name.trim().is_empty(),
            "Workspace name cannot be empty"
        );
        Ok(())
    }

    pub fn add_host(&mut self, host_id: impl Into<String>) {
        let id = host_id.into();
        if !self.host_ids.contains(&id) {
            self.host_ids.push(id);
        }
    }

    pub fn remove_host(&mut self, host_id: &str) {
        self.host_ids.retain(|h| h != host_id);
    }

    pub fn add_tag(&mut self, tag: impl Into<String>) {
        let t = tag.into().trim().to_lowercase();
        if !t.is_empty() && !self.tags.contains(&t) {
            self.tags.push(t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_management() {
        let mut ws = SessionWorkspace::new("Production Web", "Web frontend cluster");
        assert!(ws.validate().is_ok());
        ws.add_host("host-1");
        ws.add_host("host-2");
        ws.add_host("host-1"); // duplicate check
        assert_eq!(ws.host_ids.len(), 2);

        ws.add_tag("Prod");
        ws.add_tag("web");
        assert_eq!(ws.tags, vec!["prod", "web"]);

        ws.remove_host("host-1");
        assert_eq!(ws.host_ids, vec!["host-2"]);
    }
}
