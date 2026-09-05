use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditCommandEntry {
    pub id: String,
    pub session_id: String,
    pub host_id: String,
    pub host_label: String,
    pub command: String,
    pub executed_at: i64,
    pub duration_ms: Option<i64>,
    pub exit_code: Option<i32>,
}

impl AuditCommandEntry {
    pub fn new(
        session_id: impl Into<String>,
        host_id: impl Into<String>,
        host_label: impl Into<String>,
        command: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.into(),
            host_id: host_id.into(),
            host_label: host_label.into(),
            command: command.into(),
            executed_at: Utc::now().timestamp(),
            duration_ms: None,
            exit_code: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_entry() {
        let entry = AuditCommandEntry::new("sess-1", "host-1", "Production DB", "SELECT 1;");
        assert_eq!(entry.session_id, "sess-1");
        assert_eq!(entry.host_label, "Production DB");
        assert_eq!(entry.command, "SELECT 1;");
        assert!(entry.executed_at > 0);
    }
}
