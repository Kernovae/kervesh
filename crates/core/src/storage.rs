use crate::{
    AuditCommandEntry, AutomationMacro, Host, SessionWorkspace, Settings, Snippet, TriggerRule,
    TunnelConfig,
};
use anyhow::{Context, Result, bail, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Debug, PartialEq, Eq)]
pub enum Trust {
    Unknown,
    Trusted,
    Changed(String),
}
#[derive(Clone)]
pub struct Store(Arc<Mutex<Connection>>);
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    version: u32,
    hosts: Vec<Host>,
    #[serde(default)]
    settings: Settings,
    #[serde(default)]
    snippets: Vec<Snippet>,
    #[serde(default)]
    tunnels: Vec<TunnelConfig>,
    #[serde(default)]
    workspaces: Vec<SessionWorkspace>,
    #[serde(default)]
    macros: Vec<AutomationMacro>,
    #[serde(default)]
    triggers: Vec<TriggerRule>,
}

impl Store {
    pub fn default_path() -> Result<PathBuf> {
        let dirs = directories::ProjectDirs::from("org", "Kernovae", "Kervesh")
            .context("No local configuration directory")?;
        std::fs::create_dir_all(dirs.data_local_dir())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                dirs.data_local_dir(),
                std::fs::Permissions::from_mode(0o700),
            )?;
        }
        Ok(dirs.data_local_dir().join("kervesh.db"))
    }
    pub fn open(path: &Path) -> Result<Self> {
        Self::init(Connection::open(path)?)
    }
    pub fn open_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }
    fn init(connection: Connection) -> Result<Self> {
        connection.busy_timeout(std::time::Duration::from_secs(3))?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS hosts (id TEXT PRIMARY KEY, data TEXT NOT NULL, recent INTEGER NOT NULL DEFAULT 0);
            CREATE TABLE IF NOT EXISTS trust (host TEXT NOT NULL, port INTEGER NOT NULL, fingerprint TEXT NOT NULL, PRIMARY KEY(host,port));
            CREATE TABLE IF NOT EXISTS settings (id INTEGER PRIMARY KEY CHECK(id=1), data TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS snippets (id TEXT PRIMARY KEY, data TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS tunnels (id TEXT PRIMARY KEY, data TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS workspaces (id TEXT PRIMARY KEY, data TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS macros (id TEXT PRIMARY KEY, data TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS audit_commands (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, host_id TEXT NOT NULL, host_label TEXT NOT NULL, command TEXT NOT NULL, executed_at INTEGER NOT NULL, duration_ms INTEGER, exit_code INTEGER);
            CREATE INDEX IF NOT EXISTS idx_audit_executed ON audit_commands(executed_at DESC);
            CREATE TABLE IF NOT EXISTS trigger_rules (id TEXT PRIMARY KEY, data TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS vault (id INTEGER PRIMARY KEY CHECK(id=1), data BLOB NOT NULL, updated_at INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS generated_keys (id TEXT PRIMARY KEY, data TEXT NOT NULL);
            PRAGMA user_version=1;",
        )?;
        Ok(Self(Arc::new(Mutex::new(connection))))
    }
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.0
            .lock()
            .map_err(|_| anyhow::anyhow!("Local storage lock poisoned"))
    }
    pub fn hosts(&self) -> Result<Vec<Host>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare("SELECT data, recent FROM hosts ORDER BY recent DESC, id")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        let mut hosts = Vec::new();
        for row in rows {
            let (data, recent) = row?;
            let mut host: Host = serde_json::from_str(&data)?;
            host.last_connected = recent;
            hosts.push(host);
        }
        hosts.sort_by(|a, b| {
            b.favorite
                .cmp(&a.favorite)
                .then(b.last_connected.cmp(&a.last_connected))
                .then(a.name.cmp(&b.name))
        });
        Ok(hosts)
    }
    pub fn get_host(&self, id: &str) -> Result<Option<Host>> {
        let conn = self.lock()?;
        let raw: Option<(String, i64)> = conn
            .query_row("SELECT data, recent FROM hosts WHERE id=?1", [id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?;
        if let Some((data, recent)) = raw {
            let mut host: Host = serde_json::from_str(&data)?;
            host.last_connected = recent;
            Ok(Some(host))
        } else {
            Ok(None)
        }
    }
    pub fn save_host(&self, host: &Host) -> Result<()> {
        host.validate()?;
        self.lock()?.execute("INSERT INTO hosts(id,data) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET data=excluded.data", params![host.id, serde_json::to_string(host)?])?;
        Ok(())
    }
    pub fn delete_host(&self, id: &str) -> Result<()> {
        self.lock()?
            .execute("DELETE FROM hosts WHERE id=?1", [id])?;
        Ok(())
    }
    pub fn mark_connected(&self, id: &str) -> Result<()> {
        self.lock()?
            .execute("UPDATE hosts SET recent=unixepoch() WHERE id=?1", [id])?;
        Ok(())
    }
    pub fn settings(&self) -> Result<Settings> {
        let raw: Option<String> = self
            .lock()?
            .query_row("SELECT data FROM settings WHERE id=1", [], |r| r.get(0))
            .optional()?;
        let value: Settings = raw
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or_default();
        value.validate()?;
        Ok(value)
    }
    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        settings.validate()?;
        self.lock()?.execute(
            "INSERT INTO settings VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET data=excluded.data",
            [serde_json::to_string(settings)?],
        )?;
        Ok(())
    }
    pub fn snippets(&self) -> Result<Vec<Snippet>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare("SELECT data FROM snippets ORDER BY id")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut snippets = Vec::new();
        for row in rows {
            let data = row?;
            let snippet: Snippet = serde_json::from_str(&data)?;
            snippets.push(snippet);
        }
        snippets.sort_by_key(|a| a.name.to_lowercase());
        Ok(snippets)
    }
    pub fn save_snippet(&self, snippet: &Snippet) -> Result<()> {
        snippet.validate()?;
        self.lock()?.execute(
            "INSERT INTO snippets(id,data) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET data=excluded.data",
            params![snippet.id, serde_json::to_string(snippet)?],
        )?;
        Ok(())
    }
    pub fn delete_snippet(&self, id: &str) -> Result<()> {
        self.lock()?
            .execute("DELETE FROM snippets WHERE id=?1", [id])?;
        Ok(())
    }
    pub fn tunnels(&self) -> Result<Vec<TunnelConfig>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare("SELECT data FROM tunnels ORDER BY id")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut tunnels = Vec::new();
        for row in rows {
            let data = row?;
            let tunnel: TunnelConfig = serde_json::from_str(&data)?;
            tunnels.push(tunnel);
        }
        tunnels.sort_by_key(|a| a.name.to_lowercase());
        Ok(tunnels)
    }
    pub fn tunnels_for_host(&self, host_id: &str) -> Result<Vec<TunnelConfig>> {
        let all = self.tunnels()?;
        Ok(all.into_iter().filter(|t| t.host_id == host_id).collect())
    }
    pub fn save_tunnel(&self, tunnel: &TunnelConfig) -> Result<()> {
        tunnel.validate()?;
        self.lock()?.execute(
            "INSERT INTO tunnels(id,data) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET data=excluded.data",
            params![tunnel.id, serde_json::to_string(tunnel)?],
        )?;
        Ok(())
    }
    pub fn delete_tunnel(&self, id: &str) -> Result<()> {
        self.lock()?
            .execute("DELETE FROM tunnels WHERE id=?1", [id])?;
        Ok(())
    }
    pub fn workspaces(&self) -> Result<Vec<SessionWorkspace>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare("SELECT data FROM workspaces ORDER BY id")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut workspaces = Vec::new();
        for row in rows {
            let data = row?;
            let ws: SessionWorkspace = serde_json::from_str(&data)?;
            workspaces.push(ws);
        }
        workspaces.sort_by_key(|a| a.name.to_lowercase());
        Ok(workspaces)
    }
    pub fn save_workspace(&self, ws: &SessionWorkspace) -> Result<()> {
        ws.validate()?;
        self.lock()?.execute(
            "INSERT INTO workspaces(id,data) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET data=excluded.data",
            params![ws.id, serde_json::to_string(ws)?],
        )?;
        Ok(())
    }
    pub fn delete_workspace(&self, id: &str) -> Result<()> {
        self.lock()?
            .execute("DELETE FROM workspaces WHERE id=?1", [id])?;
        Ok(())
    }
    pub fn macros(&self) -> Result<Vec<AutomationMacro>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare("SELECT data FROM macros ORDER BY id")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut macros = Vec::new();
        for row in rows {
            let data = row?;
            let m: AutomationMacro = serde_json::from_str(&data)?;
            macros.push(m);
        }
        macros.sort_by_key(|a| a.name.to_lowercase());
        Ok(macros)
    }
    pub fn macros_for_host(&self, host_id: &str) -> Result<Vec<AutomationMacro>> {
        let all = self.macros()?;
        Ok(all
            .into_iter()
            .filter(|m| m.host_id.as_deref() == Some(host_id) || m.host_id.is_none())
            .collect())
    }
    pub fn save_macro(&self, m: &AutomationMacro) -> Result<()> {
        m.validate()?;
        self.lock()?.execute(
            "INSERT INTO macros(id,data) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET data=excluded.data",
            params![m.id, serde_json::to_string(m)?],
        )?;
        Ok(())
    }
    pub fn delete_macro(&self, id: &str) -> Result<()> {
        self.lock()?
            .execute("DELETE FROM macros WHERE id=?1", [id])?;
        Ok(())
    }
    pub fn triggers(&self) -> Result<Vec<TriggerRule>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare("SELECT data FROM trigger_rules ORDER BY id")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut triggers = Vec::new();
        for row in rows {
            let data = row?;
            let t: TriggerRule = serde_json::from_str(&data)?;
            triggers.push(t);
        }
        triggers.sort_by_key(|a| a.name.to_lowercase());
        Ok(triggers)
    }
    pub fn triggers_for_host(&self, host_id: &str) -> Result<Vec<TriggerRule>> {
        let all = self.triggers()?;
        Ok(all
            .into_iter()
            .filter(|t| t.host_id.as_deref() == Some(host_id) || t.host_id.is_none())
            .collect())
    }
    pub fn save_trigger(&self, t: &TriggerRule) -> Result<()> {
        self.lock()?.execute(
            "INSERT INTO trigger_rules(id,data) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET data=excluded.data",
            params![t.id, serde_json::to_string(t)?],
        )?;
        Ok(())
    }
    pub fn delete_trigger(&self, id: &str) -> Result<()> {
        self.lock()?
            .execute("DELETE FROM trigger_rules WHERE id=?1", [id])?;
        Ok(())
    }
    pub fn save_audit_command(&self, entry: &AuditCommandEntry) -> Result<()> {
        self.lock()?.execute(
            "INSERT INTO audit_commands(id,session_id,host_id,host_label,command,executed_at,duration_ms,exit_code) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                entry.id,
                entry.session_id,
                entry.host_id,
                entry.host_label,
                entry.command,
                entry.executed_at,
                entry.duration_ms,
                entry.exit_code,
            ],
        )?;
        Ok(())
    }
    pub fn audit_commands(&self, limit: usize) -> Result<Vec<AuditCommandEntry>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id,session_id,host_id,host_label,command,executed_at,duration_ms,exit_code FROM audit_commands ORDER BY executed_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |r| {
            Ok(AuditCommandEntry {
                id: r.get(0)?,
                session_id: r.get(1)?,
                host_id: r.get(2)?,
                host_label: r.get(3)?,
                command: r.get(4)?,
                executed_at: r.get(5)?,
                duration_ms: r.get(6)?,
                exit_code: r.get(7)?,
            })
        })?;
        let mut list = Vec::new();
        for row in rows {
            list.push(row?);
        }
        Ok(list)
    }
    pub fn search_audit_commands(
        &self,
        query: &str,
        host_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<AuditCommandEntry>> {
        let conn = self.lock()?;
        let pattern = format!("%{}%", query);
        let mut list = Vec::new();
        if let Some(hid) = host_id {
            let mut stmt = conn.prepare(
                "SELECT id,session_id,host_id,host_label,command,executed_at,duration_ms,exit_code FROM audit_commands WHERE (command LIKE ?1 OR host_label LIKE ?1) AND host_id = ?2 ORDER BY executed_at DESC LIMIT ?3",
            )?;
            let rows = stmt.query_map(params![pattern, hid, limit as i64], |r| {
                Ok(AuditCommandEntry {
                    id: r.get(0)?,
                    session_id: r.get(1)?,
                    host_id: r.get(2)?,
                    host_label: r.get(3)?,
                    command: r.get(4)?,
                    executed_at: r.get(5)?,
                    duration_ms: r.get(6)?,
                    exit_code: r.get(7)?,
                })
            })?;
            for row in rows {
                list.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id,session_id,host_id,host_label,command,executed_at,duration_ms,exit_code FROM audit_commands WHERE command LIKE ?1 OR host_label LIKE ?1 ORDER BY executed_at DESC LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![pattern, limit as i64], |r| {
                Ok(AuditCommandEntry {
                    id: r.get(0)?,
                    session_id: r.get(1)?,
                    host_id: r.get(2)?,
                    host_label: r.get(3)?,
                    command: r.get(4)?,
                    executed_at: r.get(5)?,
                    duration_ms: r.get(6)?,
                    exit_code: r.get(7)?,
                })
            })?;
            for row in rows {
                list.push(row?);
            }
        }
        Ok(list)
    }
    pub fn clear_audit_commands(&self) -> Result<()> {
        self.lock()?.execute("DELETE FROM audit_commands", [])?;
        Ok(())
    }
    pub fn save_vault_blob(&self, blob: &[u8]) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        self.lock()?.execute(
            "INSERT INTO vault(id,data,updated_at) VALUES(1,?1,?2) ON CONFLICT(id) DO UPDATE SET data=excluded.data, updated_at=excluded.updated_at",
            params![blob, now],
        )?;
        Ok(())
    }
    pub fn load_vault_blob(&self) -> Result<Option<Vec<u8>>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare("SELECT data FROM vault WHERE id=1")?;
        let res = stmt.query_row([], |r| r.get::<_, Vec<u8>>(0)).optional()?;
        Ok(res)
    }
    pub fn clear_vault(&self) -> Result<()> {
        self.lock()?.execute("DELETE FROM vault WHERE id=1", [])?;
        Ok(())
    }
    pub fn save_generated_key(&self, key: &crate::GeneratedKeypair) -> Result<()> {
        self.lock()?.execute(
            "INSERT INTO generated_keys(id,data) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET data=excluded.data",
            params![key.id, serde_json::to_string(key)?],
        )?;
        Ok(())
    }
    pub fn generated_keys(&self) -> Result<Vec<crate::GeneratedKeypair>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare("SELECT data FROM generated_keys ORDER BY id")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut keys = Vec::new();
        for row in rows {
            let data = row?;
            let k: crate::GeneratedKeypair = serde_json::from_str(&data)?;
            keys.push(k);
        }
        keys.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        Ok(keys)
    }
    pub fn delete_generated_key(&self, id: &str) -> Result<()> {
        self.lock()?
            .execute("DELETE FROM generated_keys WHERE id=?1", [id])?;
        Ok(())
    }
    pub fn export(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(&Export {
            version: 1,
            hosts: self.hosts()?,
            settings: self.settings()?,
            snippets: self.snippets()?,
            tunnels: self.tunnels()?,
            workspaces: self.workspaces()?,
            macros: self.macros()?,
            triggers: self.triggers()?,
        })?)
    }
    pub fn import(&self, json: &str) -> Result<usize> {
        ensure!(json.len() <= 10 * 1024 * 1024, "Import exceeds 10 MB");
        let mut data: Export = serde_json::from_str(json)?;
        ensure!(data.version == 1, "Unsupported export version");
        ensure!(data.hosts.len() <= 10000, "Too many imported hosts");
        data.settings.validate()?;
        for h in &mut data.hosts {
            h.validate()?;
            h.id = uuid::Uuid::new_v4().to_string();
        }
        for s in &mut data.snippets {
            s.validate()?;
            s.id = uuid::Uuid::new_v4().to_string();
        }
        for t in &mut data.tunnels {
            t.validate()?;
            t.id = uuid::Uuid::new_v4().to_string();
        }
        for w in &mut data.workspaces {
            w.validate()?;
            w.id = uuid::Uuid::new_v4().to_string();
        }
        for m in &mut data.macros {
            m.validate()?;
            m.id = uuid::Uuid::new_v4().to_string();
        }
        for tr in &mut data.triggers {
            tr.id = uuid::Uuid::new_v4().to_string();
        }
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        for h in &data.hosts {
            tx.execute(
                "INSERT INTO hosts(id,data) VALUES(?1,?2)",
                params![h.id, serde_json::to_string(h)?],
            )?;
        }
        for s in &data.snippets {
            tx.execute(
                "INSERT INTO snippets(id,data) VALUES(?1,?2)",
                params![s.id, serde_json::to_string(s)?],
            )?;
        }
        for t in &data.tunnels {
            tx.execute(
                "INSERT INTO tunnels(id,data) VALUES(?1,?2)",
                params![t.id, serde_json::to_string(t)?],
            )?;
        }
        for w in &data.workspaces {
            tx.execute(
                "INSERT INTO workspaces(id,data) VALUES(?1,?2)",
                params![w.id, serde_json::to_string(w)?],
            )?;
        }
        for m in &data.macros {
            tx.execute(
                "INSERT INTO macros(id,data) VALUES(?1,?2)",
                params![m.id, serde_json::to_string(m)?],
            )?;
        }
        for tr in &data.triggers {
            tx.execute(
                "INSERT INTO trigger_rules(id,data) VALUES(?1,?2)",
                params![tr.id, serde_json::to_string(tr)?],
            )?;
        }
        tx.execute(
            "INSERT INTO settings VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET data=excluded.data",
            [serde_json::to_string(&data.settings)?],
        )?;
        tx.commit()?;
        Ok(data.hosts.len())
    }
    pub fn import_ssh_config_with_report(
        &self,
        content: &str,
    ) -> Result<crate::ssh_config::OpenSshImportReport> {
        ensure!(content.len() <= 10 * 1024 * 1024, "File exceeds 10 MB");
        let report = crate::ssh_config::parse_ssh_config_with_report(content);
        if report.hosts.is_empty() {
            return Ok(report);
        }
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        for h in &report.hosts {
            tx.execute(
                "INSERT INTO hosts(id,data) VALUES(?1,?2)",
                params![h.id, serde_json::to_string(h)?],
            )?;
        }
        tx.commit()?;
        Ok(report)
    }
    pub fn import_ssh_config(&self, content: &str) -> Result<usize> {
        let report = self.import_ssh_config_with_report(content)?;
        Ok(report.hosts.len())
    }
    pub fn import_default_ssh_config(&self) -> Result<usize> {
        let path = crate::ssh_config::default_ssh_config_path()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        ensure!(path.exists(), "~/.ssh/config does not exist");
        let content = std::fs::read_to_string(&path)?;
        self.import_ssh_config(&content)
    }
    pub fn check_trust(&self, host: &str, port: u16, fingerprint: &str) -> Result<Trust> {
        let known: Option<String> = self
            .lock()?
            .query_row(
                "SELECT fingerprint FROM trust WHERE host=?1 AND port=?2",
                params![host.to_lowercase(), port],
                |r| r.get(0),
            )
            .optional()?;
        Ok(match known {
            None => Trust::Unknown,
            Some(f) if f == fingerprint => Trust::Trusted,
            Some(f) => Trust::Changed(f),
        })
    }
    pub fn trust(&self, host: &str, port: u16, fingerprint: &str) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR IGNORE INTO trust VALUES(?1,?2,?3)",
            params![host.to_lowercase(), port, fingerprint],
        )?;
        let current: String = conn.query_row(
            "SELECT fingerprint FROM trust WHERE host=?1 AND port=?2",
            params![host.to_lowercase(), port],
            |r| r.get(0),
        )?;
        if current != fingerprint {
            bail!("Host key changed; remove previous trust explicitly before reconnecting");
        }
        Ok(())
    }
    pub fn known_hosts(&self) -> Result<Vec<(String, u16, String)>> {
        let conn = self.lock()?;
        let mut stmt =
            conn.prepare("SELECT host,port,fingerprint FROM trust ORDER BY host,port")?;
        Ok(stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?)
    }
    pub fn forget_trust(&self, host: &str, port: u16) -> Result<()> {
        self.lock()?.execute(
            "DELETE FROM trust WHERE host=?1 AND port=?2",
            params![host.to_lowercase(), port],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tunnel::{TunnelConfig, TunnelKind};

    #[test]
    fn test_storage_tunnels() {
        let store = Store::open_memory().unwrap();
        let t1 = TunnelConfig::new("h1", "Web Local", TunnelKind::Local, 8080, "127.0.0.1", 80);
        let t2 = TunnelConfig::new("h1", "Socks", TunnelKind::Dynamic, 1080, "", 0);
        let t3 = TunnelConfig::new("h2", "Db", TunnelKind::Local, 5433, "127.0.0.1", 5432);

        store.save_tunnel(&t1).unwrap();
        store.save_tunnel(&t2).unwrap();
        store.save_tunnel(&t3).unwrap();

        let all = store.tunnels().unwrap();
        assert_eq!(all.len(), 3);

        let h1_tunnels = store.tunnels_for_host("h1").unwrap();
        assert_eq!(h1_tunnels.len(), 2);

        store.delete_tunnel(&t1.id).unwrap();
        assert_eq!(store.tunnels().unwrap().len(), 2);

        let exported = store.export().unwrap();
        let store2 = Store::open_memory().unwrap();
        store2.import(&exported).unwrap();
        assert_eq!(store2.tunnels().unwrap().len(), 2);
    }

    #[test]
    fn test_storage_workspaces_and_macros() {
        let store = Store::open_memory().unwrap();
        let mut ws = SessionWorkspace::new("Prod Cluster", "Primary cluster");
        ws.add_host("h1");
        ws.add_host("h2");
        ws.add_tag("production");
        store.save_workspace(&ws).unwrap();

        let all_ws = store.workspaces().unwrap();
        assert_eq!(all_ws.len(), 1);
        assert_eq!(all_ws[0].name, "Prod Cluster");
        assert_eq!(all_ws[0].host_ids, vec!["h1", "h2"]);

        let mut mac = AutomationMacro::new("Auto-Init", "Initialize session");
        mac.steps.push(crate::MacroStep::SendText {
            text: "uname -a".into(),
            append_newline: true,
        });
        mac.steps.push(crate::MacroStep::DelayMs(100));
        store.save_macro(&mac).unwrap();

        let all_mac = store.macros().unwrap();
        assert_eq!(all_mac.len(), 1);
        assert_eq!(all_mac[0].steps.len(), 2);

        let exported = store.export().unwrap();
        let store3 = Store::open_memory().unwrap();
        store3.import(&exported).unwrap();
        assert_eq!(store3.workspaces().unwrap().len(), 1);
        assert_eq!(store3.macros().unwrap().len(), 1);
    }

    #[test]
    fn test_storage_audit_and_triggers() {
        let store = Store::open_memory().unwrap();

        let mut entry1 = AuditCommandEntry::new("s1", "h1", "Host Alpha", "docker ps");
        entry1.exit_code = Some(0);
        entry1.duration_ms = Some(150);
        let mut entry2 = AuditCommandEntry::new("s2", "h2", "Host Beta", "cat /var/log/syslog");
        entry2.exit_code = Some(1);

        store.save_audit_command(&entry1).unwrap();
        store.save_audit_command(&entry2).unwrap();

        let list = store.audit_commands(10).unwrap();
        assert_eq!(list.len(), 2);

        let search_docker = store.search_audit_commands("docker", None, 10).unwrap();
        assert_eq!(search_docker.len(), 1);
        assert_eq!(search_docker[0].command, "docker ps");

        let search_host = store.search_audit_commands("", Some("h2"), 10).unwrap();
        assert_eq!(search_host.len(), 1);
        assert_eq!(search_host[0].host_id, "h2");

        let rule = TriggerRule::new(
            "Alert Trigger",
            "FATAL",
            false,
            crate::TriggerAction::Notification("Fatal error detected".into()),
        );
        store.save_trigger(&rule).unwrap();
        let triggers = store.triggers().unwrap();
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].name, "Alert Trigger");

        let exported = store.export().unwrap();
        let store2 = Store::open_memory().unwrap();
        store2.import(&exported).unwrap();
        assert_eq!(store2.triggers().unwrap().len(), 1);

        store.delete_trigger(&rule.id).unwrap();
        assert_eq!(store.triggers().unwrap().len(), 0);

        store.clear_audit_commands().unwrap();
        assert_eq!(store.audit_commands(10).unwrap().len(), 0);
    }

    #[test]
    fn test_storage_vault_and_keys() {
        let store = Store::open_memory().unwrap();

        // Vault blob
        assert_eq!(store.load_vault_blob().unwrap(), None);
        store.save_vault_blob(b"ENCRYPTED_VAULT_BYTES").unwrap();
        assert_eq!(
            store.load_vault_blob().unwrap(),
            Some(b"ENCRYPTED_VAULT_BYTES".to_vec())
        );

        // Generated keys
        let key = crate::GeneratedKeypair {
            id: "k1".into(),
            algorithm: crate::KeyAlgorithm::Ed25519,
            comment: "test-key".into(),
            public_key_openssh: "ssh-ed25519 AAAA...".into(),
            private_key_openssh: "-----BEGIN...".into(),
            fingerprint_sha256: "SHA256:abcd".into(),
            created_at: 123456789,
        };

        store.save_generated_key(&key).unwrap();
        let keys = store.generated_keys().unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].comment, "test-key");

        store.delete_generated_key(&key.id).unwrap();
        assert_eq!(store.generated_keys().unwrap().len(), 0);
    }
}
