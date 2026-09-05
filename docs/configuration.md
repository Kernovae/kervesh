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
scrollback 0–100,000 lines; monitoring 1–300 seconds. Saving terminal profiles updates scrollback in sessions using those profiles.
Existing sessions retain their initial monitoring interval until reconnected. All metrics currently
share the configured interval. Configuration writes use SQLite transactions.

Remote listing timestamps are Unix seconds. SFTP paths always use `/`, independent
of the client operating system. New/renamed names must be one path component.

## Terminal Foundation extension

Settings now include `terminal_profiles` (1–64 profiles) and
`default_terminal_profile` (an ID in that collection). Hosts optionally include
`terminal_profile`; null/missing uses the global default. A missing host reference
also falls back to the global default. Duplicate terminal IDs, missing defaults,
invalid bounds and unknown fields are rejected. Profiles are stored in the existing
SQLite settings JSON; no SQL schema migration is needed.

Old JSON remains readable. When `terminal_profiles` is absent, legacy `font_size`
and `scrollback` initialize the Default profile. These legacy fields remain in
exports for compatibility, but the terminal profile controls current rendering.
Older Kervesh versions reject the newly added fields; keep a pre-upgrade export
if downgrading. No terminal contents or clipboard data are persisted.

Each terminal profile contains:

| Field | Values / bounds |
|---|---|
| `id`, `name` | Unique ID, 1–128 ASCII letters/digits/hyphen/underscore; nonempty name ≤128 bytes |
| `font_family`, `font_fallbacks` | `Hack` or absolute local TTF/OTF path; up to eight ordered fallbacks |
| `font_size`, `line_height`, `padding` | 8–32 points; 1–2 multiplier; 0–32 points |
| `cursor_style`, `cursor_blink` | `Block`, `Beam`, `Underline`; boolean |
| `scrollback` | 0–100000 lines |
| `clipboard_profile` | `Desktop`, `Traditional` |
| `copy_on_select`, `literal_control_keys` | Boolean; literal controls use Ctrl+Alt+letter |
| `multiline_paste_policy` | `Off`, `Warn`, `AlwaysPreview` |
| `palette` | `kind`, RGB `background`, `foreground`, `cursor`, `selection`, 16 RGB `ansi` entries |
| `bell_visual`, `bell_audio`, `hyperlinks_enabled` | Boolean |
| `follow_terminal_directory` | Boolean, default false |

Palette kinds: `KerveshDark`, `KerveshLight`, `Custom`. RGB arrays are authoritative;
the UI fills built-in values when selecting a built-in palette. ANSI indices
16–255 use the standard color cube/grayscale. Remote true colors and OSC palette
changes remain authoritative. App `dark` does not change terminal colors.

Built-ins: Default, Server Administration, Development, Database, Minimal. All are
editable. Changing the active session profile does not write host configuration;
“Save profile to host” does. Saving Settings persists edited profiles and applies
them to matching active sessions. Font paths are portable references, not font data.
