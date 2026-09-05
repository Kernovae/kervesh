# Architecture & Technical Design

Kervesh is an engineer-centric, native remote systems workspace designed for Linux and Windows. It is implemented in pure, safe Rust with zero web runtime, zero Electron overhead, zero cloud dependency, and zero telemetry.

```text
+-------------------------------------------------------------------------------+
|                               kervesh (crates/app)                            |
|    egui / eframe GUI  *  Multi-Tab Workspace  *  SFTP Browser  *  DevOps Hub  |
+----------------------+--------------------------+-----------------------------+
                       |                          |
        +--------------v---------------+   +------v----------------------+
        |      kervesh-terminal        |   |         kervesh-ssh         |
        |  Alacritty VT / ANSI Engine  |   |   Tokio Runtime & russh     |
        |  Custom GPU Cell Renderer    |   |   PTY / SFTP / Channels     |
        |  Clipboard & Search Manager  |   |   SOCKS5 & X11 Bridges      |
        +--------------+---------------+   +------+----------------------+
                       |                          |
                       +--------------+-----------+
                                      |
                       +--------------v---------------+
                       |         kervesh-core         |
                       |  Domain Models & Persistence |
                       |  AES-256-GCM Encrypted Vault |
                       |  Ed25519 / RSA Key Generator |
                       |  Sync & Trigger Engine       |
                       +------------------------------+
```

---

## 1. Crates & Subsystem Responsibilities

### `kervesh-core`
Domain logic, security primitives, and local storage:
- **Data Models**: Typed host configurations, session groupings, terminal profiles, color palettes, tunnels, snippets, and macros.
- **Encrypted Vault**: Zero-knowledge master-password encrypted storage utilizing PBKDF2-HMAC-SHA256 (100,000 iterations) and AES-256-GCM AEAD authenticated encryption.
- **Keypair Generator**: In-app cryptographic Ed25519, RSA-4096, and RSA-2048 keypair generation with OpenSSH formatting and SHA-256 fingerprinting.
- **Engine Services**:
  - `TriggerEngine`: Regex and substring pattern matching engine for terminal output automation.
  - `SyncEngine`: Rsync-style tree comparison engine calculating delta plans (Uploads, Downloads, Deletions, Conflicts).
  - `SearchEngine`: High-performance remote grep query compiler and output parser.
  - `OpenSshConfig`: Complete parser for OpenSSH configuration files (`ProxyJump`, `LocalForward`, `RemoteForward`, `DynamicForward`).
- **Persistence**: SQLite storage with WAL (Write-Ahead Logging) mode, schema migrations, and secret-free JSON export/import.

### `kervesh-terminal`
Terminal emulation and low-latency graphical rendering:
- **ANSI / VT State Machine**: Driven by `alacritty_terminal` for robust compliance with ANSI escape sequences, true color, alternate screen buffer, and cursor modes.
- **Text & Glyph Rendering**: Physical pixel-aligned monospace cell rasterizer supporting primary fonts, ordered font fallbacks, complex Unicode scripts, combining characters, and 2x4 Braille graphic patterns.
- **Clipboard & Input Processing**:
  - Desktop Mode (Smart Copy/Interrupt with Ctrl+C, Paste with Ctrl+V) and Traditional Mode (Literal VT control bytes).
  - Bracketed paste mode support and multiline paste confirmation safeguards.
- **Scrollback & Local Search**: In-memory searchable scrollback buffer with real-time match highlighting and jump navigation.
- **Hyperlink Detection**: Automatic detection of OSC 8 embedded hyperlinks, standard web URLs, and remote filesystem paths.

### `kervesh-ssh`
Asynchronous networking, channel multiplexing, and protocol clients:
- **SSH Transport & PTY**: Asynchronous client built on `russh` managing connection negotiation, host key verification, cryptographic handshakes, and interactive pseudo-terminals.
- **Agentless Monitoring Pipeline**: Independent background SSH `exec` channels periodically querying Linux `/proc` filesystem metrics without polluting the interactive shell or installing remote daemons.
- **SFTP Engine**: Non-blocking file transfer system with streaming 64 KiB chunk pipelines, directory traversal, and transfer cancellation.
- **Tunneling & Dynamic Proxy**:
  - Local (`-L`) and Remote (`-R`) TCP port forwarding pipelines.
  - In-app RFC 1928 SOCKS5 Dynamic Proxy Server (`-D`) routing network traffic through SSH sessions.
- **X11 Bridge**: Bidirectional X11 channel forwarding with MIT-MAGIC-COOKIE-1 authentication.
- **Multi-Protocol Engine**: Native protocol adapters for Telnet (RFC 854/855/1091/1073 NAWS), Serial UART (`/dev/ttyUSB*`, `COM*`), FTP/FTPS, and RDP/VNC launcher orchestrators.

### `kervesh` (`crates/app`)
Desktop graphical interface and workspace management:
- **UI Engine**: Pure immediate-mode GUI powered by `egui` and `eframe` rendered via OpenGL / Glow.
- **Multi-Session Workspace**: Tabbed interface with split panes (Vertical / Horizontal), broadcast command bar, and cluster reconnection.
- **Integrated Tooling**:
  - Remote File Browser with syntax-highlighting text editor and atomic remote saving.
  - Interactive Process Manager with sortable columns and signal dispatching (`SIGTERM`, `SIGKILL`, `SIGHUP`).
  - Sysadmin & DevOps Dashboard: Docker container manager, systemd service unit manager, and network diagnostics runner (Ping, Traceroute, Port Scan, DNS).
  - Theme Engine: Interactive ANSI palette creator with real-time WCAG AA contrast ratio validation.

---

## 2. Concurrency & Data Flow Model

```text
[ GUI Thread (egui) ] <--- Event / State Sync Channels ---> [ Async Runtime (Tokio) ]
         |                                                               |
    User Actions                                                +--------+--------+
         |                                                      |                 |
  Terminal Render                                           PTY Stream       SFTP Stream
         |                                                      |                 |
  OpenGL Display                                            SSH Channel      SSH Channel
```

1. **Decoupled Architecture**: The GUI main thread operates strictly as an immediate-mode renderer and event dispatcher. Heavy operations (network I/O, file transfers, cryptography, regex scans) execute entirely on the Tokio asynchronous background worker pool.
2. **Channel Communication**: Communication between the UI and async subsystems uses bounded `tokio::sync::mpsc` channels and lock-free atomic queues.
3. **Zero RAM Bloat Transfers**: File transfers stream data in fixed 64 KiB buffers directly between disk and socket, ensuring multi-gigabyte transfers consume minimal constant memory.

---

## 3. Platform Boundaries & Standards

| Target | Windowing & Graphics | Credential Store | Serial Ports |
|---|---|---|---|
| **Linux** | X11 / Wayland via EGL / Glow | FreeDesktop Secret Service (GNOME Keyring / KWallet) | `/dev/ttyUSB*`, `/dev/ttyACM*`, `/dev/pts/*` |
| **Windows** | Win32 / Winit via WGL / Glow | Windows Credential Manager | `COM1` – `COM256` |
