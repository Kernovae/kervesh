use anyhow::{Context, Result};
use zeroize::Zeroizing;

/// Secrets never implement Debug or Serialize. They remain separate from host configuration.
#[derive(Default, Clone)]
pub struct Credentials {
    pub secret: Zeroizing<String>,
    pub remember: bool,
}

fn entry(id: &str) -> Result<keyring::Entry> {
    uuid::Uuid::parse_str(id)?;
    Ok(keyring::Entry::new("org.kernovae.kervesh", id)?)
}
pub fn load(id: &str) -> Result<Option<Zeroizing<String>>> {
    match entry(id)?.get_password() {
        Ok(secret) => Ok(Some(Zeroizing::new(secret))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => {
            Err(e).context("OS credential store unavailable; enter a session-only credential")
        }
    }
}
pub fn save(id: &str, secret: &str) -> Result<()> {
    entry(id)?
        .set_password(secret)
        .context("Could not save to OS credential store")
}
pub fn delete(id: &str) -> Result<()> {
    match entry(id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}
