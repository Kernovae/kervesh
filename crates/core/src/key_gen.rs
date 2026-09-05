use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use ssh_key::{Algorithm, HashAlg, LineEnding, PrivateKey, PublicKey};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum KeyAlgorithm {
    #[default]
    Ed25519,
    Rsa4096,
    Rsa2048,
}

impl KeyAlgorithm {
    pub fn display_name(&self) -> &'static str {
        match self {
            KeyAlgorithm::Ed25519 => "Ed25519 (Recommended, 256-bit)",
            KeyAlgorithm::Rsa4096 => "RSA 4096-bit (Legacy Compatible)",
            KeyAlgorithm::Rsa2048 => "RSA 2048-bit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedKeypair {
    pub id: String,
    pub algorithm: KeyAlgorithm,
    pub comment: String,
    pub public_key_openssh: String,
    pub private_key_openssh: String,
    pub fingerprint_sha256: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalSshKeyInfo {
    pub filename: String,
    pub path: PathBuf,
    pub key_type: String,
    pub public_key_preview: Option<String>,
    pub fingerprint: Option<String>,
    pub has_private_key: bool,
}

impl GeneratedKeypair {
    pub fn generate(algo: KeyAlgorithm, comment: &str, passphrase: Option<&str>) -> Result<Self> {
        let mut rng = ssh_key::rand_core::UnwrapErr(ssh_key::getrandom::SysRng);
        let mut priv_key = match algo {
            KeyAlgorithm::Ed25519 => PrivateKey::random(&mut rng, Algorithm::Ed25519)?,
            KeyAlgorithm::Rsa4096 => PrivateKey::random(&mut rng, Algorithm::Rsa { hash: None })?,
            KeyAlgorithm::Rsa2048 => PrivateKey::random(&mut rng, Algorithm::Rsa { hash: None })?,
        };

        if !comment.is_empty() {
            priv_key.set_comment(comment);
        }

        let pub_key = priv_key.public_key();
        let pub_openssh = pub_key.to_openssh()?;
        let fingerprint = pub_key.fingerprint(HashAlg::Sha256).to_string();

        let priv_openssh = match passphrase {
            Some(pass) if !pass.is_empty() => {
                let encrypted = priv_key.encrypt(&mut rng, pass.as_bytes())?;
                encrypted.to_openssh(LineEnding::LF)?.to_string()
            }
            _ => priv_key.to_openssh(LineEnding::LF)?.to_string(),
        };

        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            algorithm: algo,
            comment: comment.to_string(),
            public_key_openssh: pub_openssh,
            private_key_openssh: priv_openssh,
            fingerprint_sha256: fingerprint,
            created_at: Utc::now().timestamp(),
        })
    }
}

pub fn generate_ssh_copy_id_command(public_key: &str) -> String {
    let clean_pub = public_key.trim();
    format!(
        "mkdir -p ~/.ssh && chmod 700 ~/.ssh && echo '{}' >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys",
        clean_pub.replace('\'', "'\\''")
    )
}

pub fn discover_local_ssh_keys() -> Vec<LocalSshKeyInfo> {
    let mut list = Vec::new();
    let home = match directories::BaseDirs::new() {
        Some(b) => b.home_dir().to_path_buf(),
        None => return list,
    };
    let ssh_dir = home.join(".ssh");
    if !ssh_dir.exists() || !ssh_dir.is_dir() {
        return list;
    }

    let candidates = [
        "id_ed25519",
        "id_rsa",
        "id_ecdsa",
        "id_dsa",
        "identity",
        "id_ed25519_sk",
        "id_ecdsa_sk",
    ];

    for name in &candidates {
        let priv_path = ssh_dir.join(name);
        let pub_path = ssh_dir.join(format!("{}.pub", name));

        let has_priv = priv_path.exists() && priv_path.is_file();
        let has_pub = pub_path.exists() && pub_path.is_file();

        if has_priv || has_pub {
            let mut pub_preview = None;
            let mut fingerprint = None;
            let mut key_type = name.replace("id_", "").to_uppercase();

            if has_pub && let Ok(content) = std::fs::read_to_string(&pub_path) {
                let trimmed = content.trim().to_string();
                if let Ok(parsed_pub) = PublicKey::from_openssh(&trimmed) {
                    fingerprint = Some(parsed_pub.fingerprint(HashAlg::Sha256).to_string());
                    key_type = parsed_pub.algorithm().to_string().to_uppercase();
                }
                pub_preview = Some(trimmed);
            }

            list.push(LocalSshKeyInfo {
                filename: name.to_string(),
                path: if has_priv { priv_path } else { pub_path },
                key_type,
                public_key_preview: pub_preview,
                fingerprint,
                has_private_key: has_priv,
            });
        }
    }

    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_ed25519_keypair() {
        let keypair =
            GeneratedKeypair::generate(KeyAlgorithm::Ed25519, "deploy-admin@kervesh", None)
                .unwrap();

        assert!(keypair.public_key_openssh.starts_with("ssh-ed25519 "));
        assert!(keypair.public_key_openssh.contains("deploy-admin@kervesh"));
        assert!(
            keypair
                .private_key_openssh
                .contains("-----BEGIN OPENSSH PRIVATE KEY-----")
        );
        assert!(keypair.fingerprint_sha256.starts_with("SHA256:"));
    }

    #[test]
    fn test_ssh_copy_id_command() {
        let cmd = generate_ssh_copy_id_command("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5 admin@box");
        assert!(cmd.contains("mkdir -p ~/.ssh"));
        assert!(cmd.contains("chmod 700 ~/.ssh"));
        assert!(cmd.contains("authorized_keys"));
        assert!(cmd.contains("admin@box"));
    }
}
