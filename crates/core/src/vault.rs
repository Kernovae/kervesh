use anyhow::{Context, Result, bail};
use chrono::Utc;
use ring::aead::{
    AES_256_GCM, Aad, BoundKey, Nonce, NonceSequence, OpeningKey, SealingKey, UnboundKey,
};
use ring::pbkdf2;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;

const PBKDF2_ITERATIONS: u32 = 100_000;
const SALT_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const MAGIC: &[u8; 4] = b"KRVV"; // Kervesh Vault Format

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VaultCategory {
    Password,
    SshPrivateKey,
    ApiToken,
    Note,
}

impl VaultCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            VaultCategory::Password => "Password",
            VaultCategory::SshPrivateKey => "SSH Private Key",
            VaultCategory::ApiToken => "API / Access Token",
            VaultCategory::Note => "Secure Note",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            VaultCategory::Password => "🔑",
            VaultCategory::SshPrivateKey => "🛡",
            VaultCategory::ApiToken => "⚡",
            VaultCategory::Note => "📝",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultEntry {
    pub id: String,
    pub title: String,
    pub category: VaultCategory,
    pub username: String,
    pub secret: String,
    pub notes: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl VaultEntry {
    pub fn new(
        title: impl Into<String>,
        category: VaultCategory,
        username: impl Into<String>,
        secret: impl Into<String>,
        notes: impl Into<String>,
    ) -> Self {
        let now = Utc::now().timestamp();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.into(),
            category,
            username: username.into(),
            secret: secret.into(),
            notes: notes.into(),
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct VaultData {
    entries: Vec<VaultEntry>,
}

#[derive(Debug, Clone)]
pub struct EncryptedVault {
    data: VaultData,
}

struct OneNonce(Option<[u8; NONCE_LEN]>);
impl NonceSequence for OneNonce {
    fn advance(&mut self) -> Result<Nonce, ring::error::Unspecified> {
        let n = self.0.take().ok_or(ring::error::Unspecified)?;
        Nonce::try_assume_unique_for_key(&n)
    }
}

impl EncryptedVault {
    pub fn empty() -> Self {
        Self {
            data: VaultData::default(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.data.entries.is_empty()
    }

    pub fn entries(&self) -> &[VaultEntry] {
        &self.data.entries
    }

    pub fn add_entry(&mut self, entry: VaultEntry) {
        self.data.entries.push(entry);
        self.data.entries.sort_by_key(|e| e.title.to_lowercase());
    }

    pub fn update_entry(&mut self, entry: VaultEntry) {
        if let Some(pos) = self.data.entries.iter().position(|e| e.id == entry.id) {
            self.data.entries[pos] = entry;
            self.data.entries.sort_by_key(|e| e.title.to_lowercase());
        }
    }

    pub fn delete_entry(&mut self, id: &str) {
        self.data.entries.retain(|e| e.id != id);
    }

    pub fn get_entry(&self, id: &str) -> Option<&VaultEntry> {
        self.data.entries.iter().find(|e| e.id == id)
    }

    pub fn search(&self, query: &str) -> Vec<&VaultEntry> {
        let q = query.to_lowercase();
        self.data
            .entries
            .iter()
            .filter(|e| {
                e.title.to_lowercase().contains(&q)
                    || e.username.to_lowercase().contains(&q)
                    || e.notes.to_lowercase().contains(&q)
            })
            .collect()
    }

    /// Derives 256-bit encryption key using PBKDF2-HMAC-SHA256
    fn derive_key(password: &str, salt: &[u8]) -> [u8; 32] {
        let mut key = [0u8; 32];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            NonZeroU32::new(PBKDF2_ITERATIONS).unwrap(),
            salt,
            password.as_bytes(),
            &mut key,
        );
        key
    }

    /// Encrypts vault contents into binary blob: [MAGIC(4) | SALT(32) | NONCE(12) | CIPHERTEXT + TAG]
    pub fn encrypt_to_blob(&self, master_password: &str) -> Result<Vec<u8>> {
        let rng = SystemRandom::new();
        let mut salt = [0u8; SALT_LEN];
        let mut nonce = [0u8; NONCE_LEN];
        rng.fill(&mut salt)
            .map_err(|_| anyhow::anyhow!("RNG failed for salt"))?;
        rng.fill(&mut nonce)
            .map_err(|_| anyhow::anyhow!("RNG failed for nonce"))?;

        let key_bytes = Self::derive_key(master_password, &salt);
        let unbound_key = UnboundKey::new(&AES_256_GCM, &key_bytes)
            .map_err(|_| anyhow::anyhow!("Invalid AEAD key"))?;
        let mut sealing_key = SealingKey::new(unbound_key, OneNonce(Some(nonce)));

        let plaintext = serde_json::to_vec(&self.data)?;
        let mut in_out = plaintext;
        sealing_key
            .seal_in_place_append_tag(Aad::empty(), &mut in_out)
            .map_err(|_| anyhow::anyhow!("Encryption failed"))?;

        let mut blob = Vec::with_capacity(MAGIC.len() + SALT_LEN + NONCE_LEN + in_out.len());
        blob.extend_from_slice(MAGIC);
        blob.extend_from_slice(&salt);
        blob.extend_from_slice(&nonce);
        blob.extend_from_slice(&in_out);

        Ok(blob)
    }

    /// Decrypts vault binary blob using master password
    pub fn unlock(blob: &[u8], master_password: &str) -> Result<Self> {
        let header_len = MAGIC.len() + SALT_LEN + NONCE_LEN;
        if blob.len() < header_len + AES_256_GCM.tag_len() {
            bail!("Invalid vault data: buffer too small");
        }

        if &blob[..4] != MAGIC {
            bail!("Invalid vault format: magic mismatch");
        }

        let salt = &blob[4..4 + SALT_LEN];
        let nonce_bytes: [u8; NONCE_LEN] = blob[4 + SALT_LEN..header_len]
            .try_into()
            .context("Invalid nonce size")?;

        let key_bytes = Self::derive_key(master_password, salt);
        let unbound_key = UnboundKey::new(&AES_256_GCM, &key_bytes)
            .map_err(|_| anyhow::anyhow!("Invalid AEAD key"))?;
        let mut opening_key = OpeningKey::new(unbound_key, OneNonce(Some(nonce_bytes)));

        let mut in_out = blob[header_len..].to_vec();
        let decrypted = opening_key
            .open_in_place(Aad::empty(), &mut in_out)
            .map_err(|_| anyhow::anyhow!("Incorrect master password or corrupted vault"))?;

        let data: VaultData = serde_json::from_slice(decrypted)
            .context("Failed to deserialize decrypted vault content")?;

        Ok(Self { data })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypted_vault_roundtrip() {
        let mut vault = EncryptedVault::empty();
        let entry1 = VaultEntry::new(
            "Root Database Server",
            VaultCategory::Password,
            "postgres",
            "super_secret_pw_123!",
            "Production main cluster",
        );
        let entry2 = VaultEntry::new(
            "AWS Deploy Token",
            VaultCategory::ApiToken,
            "ci-runner",
            "AKIAIOSFODNN7EXAMPLE",
            "Read-only staging",
        );

        vault.add_entry(entry1);
        vault.add_entry(entry2);

        let password = "MyUltraSecureMasterPassword!2026";
        let encrypted_blob = vault.encrypt_to_blob(password).unwrap();
        assert!(encrypted_blob.len() > 100);

        // Success unlock
        let unlocked = EncryptedVault::unlock(&encrypted_blob, password).unwrap();
        assert_eq!(unlocked.entries().len(), 2);
        assert_eq!(unlocked.entries()[0].title, "AWS Deploy Token");
        assert_eq!(unlocked.entries()[1].title, "Root Database Server");
        assert_eq!(unlocked.entries()[1].secret, "super_secret_pw_123!");

        // Incorrect password fails
        let wrong_result = EncryptedVault::unlock(&encrypted_blob, "WrongPassword!");
        assert!(wrong_result.is_err());
    }

    #[test]
    fn test_vault_search() {
        let mut vault = EncryptedVault::empty();
        vault.add_entry(VaultEntry::new(
            "GitHub Enterprise",
            VaultCategory::ApiToken,
            "octocat",
            "ghp_token_xyz",
            "Org token",
        ));
        vault.add_entry(VaultEntry::new(
            "Bastion SSH Key",
            VaultCategory::SshPrivateKey,
            "ubuntu",
            "-----BEGIN OPENSSH PRIVATE KEY-----",
            "AWS VPC jump host",
        ));

        let results = vault.search("github");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "GitHub Enterprise");

        let search_notes = vault.search("jump host");
        assert_eq!(search_notes.len(), 1);
        assert_eq!(search_notes[0].title, "Bastion SSH Key");
    }
}
