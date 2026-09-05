# Security

Host keys are verified before authentication. Unknown fingerprints require an
explicit trust decision; changed keys fail closed. Verify replacements out of
band before removing old trust. Host certificates/CA trust are not implemented.
Agent forwarding, arbitrary ProxyCommand, telemetry and cloud services are absent.

Passwords/passphrases are excluded from profile serialization, SQLite and exports.
Optional storage uses Windows Credential Manager or Linux Secret Service. Local
secrets are held in zeroizing buffers where owned; protocol/GUI dependencies may
make transient copies, so this is not a guarantee against memory inspection.
Terminal OSC 52 clipboard read/write requests are disabled. Bracketed paste strips
embedded escape characters to prevent a pasted terminator escaping the paste.

Keep private keys and the local data directory protected by OS permissions. Hosts,
usernames, paths and fingerprints are sensitive metadata even without passwords.
No logs intentionally include credentials or terminal content. Removing a profile
attempts credential cleanup; if the keyring is unavailable, remove its orphaned
UUID entry in the OS credential manager after unlocking it.

Transfers stage `.kervesh-<id>.part` files. Upload overwrite temporarily renames the
old destination to `.kervesh-<id>.backup`, installs the completed file and removes
the backup. Failed installation attempts rollback. This is not an atomic rename
exchange. A process/network crash during replacement may leave staging/backup
files; inspect and restore them before retrying. Never delete the only valid
backup. Explicit transfer cancellation attempts staging cleanup; closing a tab or
killing the process can leave partial files. Upload overwrite may replace remote
file permissions with server defaults. Symlink recursion is not performed.

Report vulnerabilities privately to repository maintainers through the hosting
provider's private vulnerability-reporting channel when available. Until such a
channel exists, do not publish exploit details, keys, credentials or host lists
in public issues. No security contact address is invented for this new repository.
