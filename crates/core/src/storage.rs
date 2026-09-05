use crate::{Host, Settings};
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
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS hosts (id TEXT PRIMARY KEY, data TEXT NOT NULL, recent INTEGER NOT NULL DEFAULT 0);
            CREATE TABLE IF NOT EXISTS trust (host TEXT NOT NULL, port INTEGER NOT NULL, fingerprint TEXT NOT NULL, PRIMARY KEY(host,port));
            CREATE TABLE IF NOT EXISTS settings (id INTEGER PRIMARY KEY CHECK(id=1), data TEXT NOT NULL);
            PRAGMA user_version=1;")?;
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
    pub fn export(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(&Export {
            version: 1,
            hosts: self.hosts()?,
            settings: self.settings()?,
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
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        for h in &data.hosts {
            tx.execute(
                "INSERT INTO hosts(id,data) VALUES(?1,?2)",
                params![h.id, serde_json::to_string(h)?],
            )?;
        }
        tx.execute(
            "INSERT INTO settings VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET data=excluded.data",
            [serde_json::to_string(&data.settings)?],
        )?;
        tx.commit()?;
        Ok(data.hosts.len())
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
