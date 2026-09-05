# Security Architecture & Cryptography

Security in Kervesh is built around local-first data isolation, zero-knowledge storage, and strict verification boundaries.

---

## 1. Zero-Knowledge Encrypted Vault

Kervesh features a built-in cryptographic vault for storing sensitive passwords, private keys, API tokens, and credentials.

```text
User Master Password
        │
        ▼
PBKDF2-HMAC-SHA256 (100,000 iterations + 32-byte cryptographically random salt)
        │
        ▼
256-bit Encryption Key
        │
        ▼
AES-256-GCM Authenticated Encryption (12-byte unique Nonce + AEAD Tag)
        │
        ▼
Encrypted Vault Blob: [ KRVV | Salt (32B) | Nonce (12B) | Ciphertext + Tag ]
```

- **Key Derivation**: PBKDF2 with HMAC-SHA256 and 100,000 rounds ensures resilience against brute-force and dictionary attacks.
- **AEAD Authenticated Encryption**: AES-256-GCM provides both confidentiality and tamper resistance. Any modification of the ciphertext causes decryption to fail.
- **Memory Protection**: Sensitive memory buffers containing private keys and decrypted secrets are zeroized in memory when dropped via the `zeroize` crate.

---

## 2. Host Key Verification & Trust Boundary

- **Strict Fingerprint Verification**: Upon establishing an SSH connection, the remote host's public key fingerprint (SHA-256) is matched against the local SQLite trust store.
- **Fail-Closed Protection**: If a host key changes, Kervesh immediately halts connection negotiation and alerts the user, preventing Man-in-the-Middle (MITM) attacks.
- **Port-Isolated Trust**: Trust records are partitioned by `(hostname, port)` tuples so different services on distinct ports are evaluated independently.

---

## 3. Operating System Keyring Integration

Credentials saved outside the encrypted vault utilize platform-native secure credential stores:
- **Linux**: FreeDesktop Secret Service D-Bus API (`org.freedesktop.secrets`), interfacing with GNOME Keyring or KWallet.
- **Windows**: Windows Credential Manager (`wincred`).
- **Session-Only Mode**: Users can opt out of keyring persistence; credentials then remain strictly in transient process memory.

---

## 4. SSH Keypair Generation & Remote Deployment

- **Cryptographically Secure RNG**: Key generation uses operating system entropy (`getrandom` / `SysRng`).
- **Key Formats**: Generates OpenSSH-compatible Ed25519 (256-bit) and RSA (2048 / 4096-bit) keys.
- **Safe Key Deployment (`ssh-copy-id`)**: When deploying public keys to remote hosts, POSIX file permissions are enforced (`0700` for `~/.ssh` directory, `0600` for `authorized_keys`).

---

## 5. Data Privacy & Export Safeguards

- **Secret-Free Backups**: JSON configuration exports exclude passwords, private key material, and session history by design.
- **Terminal Isolation**: OSC 52 clipboard reading is restricted by default to prevent remote untrusted scripts from reading local clipboard contents.
- **No Telemetry**: Kervesh does not collect analytics, telemetry, or network crash reports.
