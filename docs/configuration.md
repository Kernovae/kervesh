# Configuration & Storage Architecture

Kervesh stores all configurations locally using SQLite with Write-Ahead Logging (WAL) and platform-native credential managers.

---

## 1. Storage Locations & Environment Variables

| Platform | Default Path |
|---|---|
| **Linux** | `~/.local/share/kervesh/` (or `$XDG_DATA_HOME/kervesh/`) |
| **Windows** | `%APPDATA%\Kernovae\Kervesh\` |

### Environment Variables
- `KERVESH_DATA_DIR`: Overrides the default data directory (useful for portable installations, testing, and isolated environments).
- `KERVESH_RECORDINGS_DIR`: Overrides the default directory for session recordings (`.cast`, `.txt`, `.raw`).

---

## 2. Portable JSON Export & Import Schema

Configurations can be exported to and imported from portable JSON files. By design, exports never contain raw passwords, private keys, or host trust fingerprints.

```json
{
  "version": 1,
  "hosts": [
    {
      "id": "1dc9a749-374d-4898-8bde-e1cbfdd54bfe",
      "name": "Production Web Node",
      "hostname": "192.0.2.10",
      "port": 22,
      "username": "deploy",
      "auth": "PrivateKey",
      "key_path": "~/.ssh/id_ed25519",
      "group": "Production",
      "tags": "web, frontend, ssl",
      "favorite": true,
      "timeout_secs": 15,
      "keepalive_secs": 30,
      "auto_reconnect": false,
      "proxy_jump": "bastion-gateway",
      "terminal_profile": "development"
    }
  ],
  "settings": {
    "dark": true,
    "font_size": 14.0,
    "scrollback": 10000,
    "monitor_secs": 2,
    "show_hidden": false,
    "sidebar": true,
    "sftp_panel": true,
    "default_terminal_profile": "default",
    "terminal_profiles": []
  }
}
```

### Import Validation Rules
- Imports over 10 MiB or exceeding 10,000 host records are rejected before execution.
- Imported hosts receive fresh UUIDs to prevent collision with existing hosts.
- Corrupted schemas or invalid port numbers cause atomic transaction rollback.

---

## 3. Terminal Profiles Schema

Terminal settings are organized into customizable profiles:

| Field | Type | Description |
|---|---|---|
| `id` | String | Unique profile identifier (e.g. `default`, `dev`, `server-admin`). |
| `name` | String | User-facing profile label. |
| `font_family` | String | Font family name (e.g. `Hack`) or absolute TTF/OTF path. |
| `font_fallbacks` | Array | Up to 8 ordered fallback font paths. |
| `font_size` | Float | Terminal font size (8.0 to 32.0 pt). |
| `line_height` | Float | Line height multiplier (1.0 to 2.0). |
| `padding` | Float | Monospace grid inner padding (0.0 to 32.0 pt). |
| `cursor_style` | Enum | `Block`, `Beam`, or `Underline`. |
| `cursor_blink` | Boolean | Enables or disables cursor blinking animation. |
| `scrollback` | Integer | Scrollback buffer capacity in lines (up to 100,000). |
| `clipboard_profile` | Enum | `Desktop` (Smart Ctrl+C/V) or `Traditional` (Literal control bytes). |
| `multiline_paste_policy` | Enum | `Off`, `Warn`, or `AlwaysPreview`. |
| `palette` | Object | Full ANSI 16-color palette + background/foreground/cursor/selection. |
| `bell_visual` | Boolean | Flashes terminal screen on ASCII 0x07 bell. |
| `bell_audio` | Boolean | Emits system alert sound on ASCII 0x07 bell. |
| `hyperlinks_enabled` | Boolean | Enables OSC 8 and URL auto-detection. |

---

## 4. Key Bindings & Shortcuts

| Shortcut | Action | Scope |
|---|---|---|
| `Ctrl+N` | Open New Connection Dialog | Global |
| `Ctrl+W` | Close Active Tab / Session | Global |
| `Ctrl+Tab` | Next Session Tab | Global |
| `Ctrl+Shift+Tab` | Previous Session Tab | Global |
| `Ctrl+Shift+D` | Split Pane Vertically | Terminal Workspace |
| `Ctrl+Shift+E` | Split Pane Horizontally | Terminal Workspace |
| `Ctrl+F` | Open In-Terminal Search Bar | Terminal |
| `Ctrl+Shift+C` | Copy Selection to System Clipboard | Terminal |
| `Ctrl+Shift+V` | Paste from System Clipboard | Terminal |
| `Shift+PageUp` | Scroll Up Scrollback Buffer | Terminal |
| `Shift+PageDown` | Scroll Down Scrollback Buffer | Terminal |
| `Ctrl+Click` | Open Hyperlink or Reveal SFTP Path | Terminal |
