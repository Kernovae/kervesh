# Portable configuration v1

Exports are UTF-8 JSON objects with `version: 1`, `hosts` and `settings`.
Unknown properties, unsupported versions, invalid profiles/settings, imports over
10 MiB and imports above 10,000 hosts are rejected before a transaction commits.
Imports append profiles with fresh UUIDs and replace preferences atomically.

```json
{
  "version": 1,
  "hosts": [{
    "id": "1dc9a749-374d-4898-8bde-e1cbfdd54bfe",
    "name": "Lab",
    "hostname": "192.0.2.10",
    "port": 22,
    "username": "operator",
    "auth": "PrivateKey",
    "key_path": "/home/operator/.ssh/id_ed25519",
    "group": "Homelab",
    "tags": "linux, lab",
    "favorite": true,
    "timeout_secs": 15,
    "keepalive_secs": 30,
    "auto_reconnect": false
  }],
  "settings": {
    "dark": true,
    "font_size": 14.0,
    "scrollback": 10000,
    "monitor_secs": 2,
    "show_hidden": false,
    "sidebar": true,
    "sftp_panel": true
  }
}
```

Authentication values: `Password`, `PrivateKey`, `Agent`. No password/passphrase
property exists. Key paths reference files and may need adjustment on another
machine. Credentials use keyring service `org.kernovae.kervesh` and profile UUID
as account. Trusted endpoints key on case-normalized hostname plus port, with
SHA-256 public-key fingerprints. Trust never imports or exports automatically.

Timeout 1–300 seconds; keepalive 0–3600 seconds (0 disables); font 8–32 points;
scrollback 0–100,000 lines; monitoring 1–300 seconds. Existing sessions retain
initial scrollback/polling interval until reconnected. All metrics currently
share the configured interval. Configuration writes use SQLite transactions.

Remote listing timestamps are Unix seconds. SFTP paths always use `/`, independent
of the client operating system. New/renamed names must be one path component.
