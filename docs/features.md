# Feature Guide & Capabilities

Kervesh integrates terminal emulation, remote file management, network routing, and system administration tooling into a single native workspace.

---

## 1. Remote Connectivity & Protocols

### SSH & Jump Hosts (Bastions)
- **Direct & Multi-Hop Connections**: Connect to target instances directly or chained through intermediate bastion/jump hosts (`ProxyJump`).
- **OpenSSH Config Importer**: 1-click import of `~/.ssh/config` parsing hosts, ports, users, identities, proxies, and port forwards.
- **Authentication Methods**: Passwords, encrypted/unencrypted private keys (Ed25519, RSA, ECDSA), and native SSH Agent forwarding.
- **Strict Host Key Verification**: Cryptographic SHA-256 fingerprint verification with fail-closed protection against key mismatch.

### Multi-Protocol Engine
- **Telnet Client**: Full RFC 854 / 855 / 1091 / 1073 (NAWS) implementation with dynamic window resizing and ANSI feed passthrough.
- **Serial Port (UART)**: Hardware interface supporting `/dev/ttyUSB*`, `/dev/ttyACM*`, and `COM*` ports with configurable baud rate, data bits, parity, stop bits, and flow control.
- **FTP / FTPS Storage**: RFC 959 / 3659 FTP client with PASV/EPSV data channels, recursive tree navigation, and streaming file transfers.
- **RDP & VNC Remote Desktop**: Native launcher orchestrator for `xfreerdp`, `mstsc.exe`, `vncviewer`, `tigervnc`, and `remmina` with custom resolutions and color depth.

---

## 2. Terminal Workspace & Productivity

### Terminal Engine & Display
- **Alacritty VT Core**: Full support for ANSI 256 colors, 24-bit TrueColor, alternate screen buffer, UTF-8 combining sequences, and 2x4 Braille graphic rendering.
- **Custom Monospace Typography**: Primary font family selection with up to 8 ordered font fallbacks and subpixel cell metrics.
- **Split Views**: Vertical and horizontal read-only mirror views of the active PTY output. Both views share one shell session; they are not independent panes.
- **Clipboard Profiles**:
  - *Desktop Mode*: `Ctrl+C` smart copy / interrupt, `Ctrl+V` paste, `Ctrl+Shift+C/V` shortcuts.
  - *Traditional Mode*: Literal control bytes for Unix command compatibility.
  - *Paste Protection*: Bracketed paste injection and multiline preview dialog.

### Search, Links & Session Recording
- **Local Scrollback Search**: `Ctrl+F` in-memory regex and literal search with immediate match navigation.
- **Hyperlink & Path Navigation**: Automatic detection of OSC 8 hyperlinks, standard web URLs, and remote file paths with 1-click "Reveal in SFTP".
- **Session Recording**: Multi-format session recording engine supporting Asciicast v2 (`.cast`), Clean Text (`.txt`), and Raw Byte Streams (`.raw`).
- **Trigger-Action Rules Engine**: Regex pattern monitoring on terminal output streams triggering desktop notifications, audible bells, or automated key injection. Raw PTY input is not stored as command history because it can contain passwords and control bytes.

---

## 3. Remote File Productivity & Sync

### SFTP File Browser & Transfers
- **Visual Remote File Manager**: Directory traversal, permissions (`chmod`), file creation, renaming, deletion, and drag-and-drop file upload.
- **Streaming Transfer Engine**: Non-blocking asynchronous transfers with real-time throughput metrics, progress indicators, and cancellation.
- **Integrated Remote Editor**: Text editor with syntax highlighting detection, line endings toggle (LF / CRLF), line numbers, find & replace, and atomic remote file saving.

### File Search & Directory Synchronization
- **Remote Grep & Text Search**: Deep remote recursive search via background execution with regex, case-sensitivity, and file extension filters.
- **Directory Synchronization**: Rsync-style local-to-remote, remote-to-local, and bidirectional tree comparison with diff preview and batch execution.

---

## 4. Network Tunnels & Forwarding

- **Port Forwarding**: Local (`-L`) and Remote (`-R`) TCP port forwarding with live throughput and connection counters.
- **SOCKS5 Dynamic Proxy**: Built-in RFC 1928 SOCKS5 proxy server (`-D`) for routing browser and third-party application traffic securely through remote hosts.
- **X11 Forwarding**: Secure bidirectional X11 tunneling with MIT-MAGIC-COOKIE-1 authentication for remote graphical application execution.

---

## 5. Sysadmin & DevOps Hub

- **Docker Container & Image Manager**: Live container inspection, real-time log streaming, lifecycle operations (Start, Stop, Restart), and image repository inventory.
- **Systemd Service Manager**: Service unit status inspection, active/inactive/failed state badges, `journalctl` log viewer, and service control.
- **Network Diagnostics Hub**: Built-in Ping, Traceroute, Port Reachability Scan, and DNS Resolution utilities executed directly inside remote session contexts.
- **Process Manager**: Live process table (`ps -eo`) with sortable columns, user filtering, and POSIX signal dispatching (`SIGTERM`, `SIGKILL`, `SIGHUP`, `SIGINT`).
- **Snippets & Multi-Exec**: Reusable parameterized command snippets (`{{placeholders}}`) and broadcast bar for dispatching commands across all connected cluster nodes.

---

## 6. Security & Personalization

- **Zero-Knowledge Encrypted Vault**: Master-password encrypted database using PBKDF2-HMAC-SHA256 (100,000 iterations) and AES-256-GCM AEAD encryption.
- **SSH Keypair Generator**: In-app cryptographic Ed25519 and RSA key generator with passphrase protection and 1-click deployment to remote `~/.ssh/authorized_keys`.
- **Theme Engine & ANSI Palette Editor**: Visual theme designer with built-in presets (Dracula, Nord, Tokyo Night, Catppuccin Mocha, Gruvbox, One Dark, Solarized, Monokai, Cyberpunk), live preview, JSON export/import, and real-time WCAG AA contrast ratio checking.
