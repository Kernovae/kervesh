# Kervesh

**Your hosts. Your keys. Your machine.**

Kervesh is a high-performance, native remote systems workspace for Linux and Windows. It combines hardware-accelerated terminal emulation, SFTP file management, SSH tunnels, SOCKS5 dynamic proxying, multi-protocol remote connectivity, and sysadmin tooling into a single lightweight desktop application.

Implemented in pure Rust with zero Electron, zero browser runtimes, zero cloud dependencies, and zero telemetry.

---

## Highlights

- **Multi-Protocol Connectivity**: Native SSH (with multi-hop Bastion jump host cascading and `~/.ssh/config` import), Telnet, Serial UART ports, FTP/FTPS, and RDP/VNC remote desktop orchestrators.
- **High-Performance Terminal**: Alacritty VT/ANSI state machine, GPU text rasterization, true color support, customizable typography with fallback chains, split panes (Vertical/Horizontal), and broadcast command execution.
- **SFTP File Browser & Remote Editor**: Visual remote file manager with streaming transfers, directory synchronization (rsync-style delta engine), remote grep search, and built-in code editor with syntax highlighting and atomic saving.
- **Network Routing & Tunnels**: Visual SSH port forwarding dashboard (Local `-L`, Remote `-R`), in-app SOCKS5 dynamic forwarding proxy (`-D`), and X11 forwarding with `MIT-MAGIC-COOKIE-1` authentication.
- **Sysadmin & DevOps Toolbox**: Docker container manager, systemd service unit manager with `journalctl` log viewer, remote process manager with POSIX signal dispatching, and network diagnostics hub (Ping, Traceroute, Port Scan, DNS).
- **Security & Cryptography**: Zero-knowledge encrypted vault (PBKDF2-HMAC-SHA256 + AES-256-GCM AEAD), Ed25519/RSA SSH keypair generator with `ssh-copy-id` deployment, and platform keyring integration.
- **Terminal Intelligence**: Multi-format session recording (`.cast`, `.txt`, `.raw`), searchable command audit history, and customizable trigger-action rules engine.
- **Workspace Personalization**: Visual theme designer with ANSI palette editor, built-in presets (Dracula, Nord, Tokyo Night, Catppuccin, Gruvbox, One Dark, etc.), live terminal preview, and real-time WCAG AA contrast ratio checking.

---

## Quickstart

### Prerequisites
- **Rust 1.92+**
- Desktop session with OpenGL / EGL support

### Build & Run

```bash
# Run in debug mode:
cargo run -p kervesh

# Build optimized release binary:
cargo build --release -p kervesh
./target/release/kervesh
```

### Linux Build Dependencies

**Debian / Ubuntu**:
```bash
sudo apt-get update && sudo apt-get install -y \
  build-essential pkg-config libx11-dev libxkbcommon-dev \
  libxkbcommon-x11-0 libwayland-dev libegl1-mesa-dev libgl1-mesa-dev
```

**Fedora / RHEL**:
```bash
sudo dnf install -y \
  gcc gcc-c++ make pkgconf-pkg-config libX11-devel \
  libxkbcommon-devel wayland-devel mesa-libEGL-devel mesa-libGL-devel
```

### Windows Build Requirements
Build using the Rust MSVC toolchain (`x86_64-pc-windows-msvc`) with Visual Studio C++ build tools.

---

## Architecture

Kervesh is structured as a modular Cargo workspace:

| Crate | Path | Responsibility |
|---|---|---|
| **`kervesh-core`** | [`crates/core`](crates/core) | Domain models, SQLite storage, AES-256-GCM vault, keygen, sync engine, triggers |
| **`kervesh-terminal`** | [`crates/terminal`](crates/terminal) | Alacritty VT engine, GPU text rendering, clipboard policies, local search |
| **`kervesh-ssh`** | [`crates/ssh`](crates/ssh) | Tokio async transport, SSH/PTY multiplexing, SFTP streaming, SOCKS5 proxy, X11 bridge |
| **`kervesh`** | [`crates/app`](crates/app) | Native egui/eframe desktop application, workspaces, SFTP UI, devops tools, themes |

---

## Documentation

- **[Architecture & Technical Design](docs/architecture.md)**: In-depth design of crates, concurrency model, and data flow.
- **[Feature Guide & Capabilities](docs/features.md)**: User guide covering all protocols, tools, and workflows.
- **[Security Architecture](docs/security.md)**: Zero-knowledge vault, key derivation, and host trust boundaries.
- **[Configuration & Storage](docs/configuration.md)**: File formats, schemas, terminal profiles, and key shortcuts.
- **[Branching & Governance](docs/branching.md)**: Repository branch model and CI quality gates.

---

## Quality & Verification

Run the comprehensive test suite across all 21 test suites:

```bash
# Code formatting check
cargo fmt --all -- --check

# Strict compiler and clippy lints
cargo clippy --workspace --all-targets -- -D warnings

# Unit, integration, and high-concurrency stress tests (100+ tests)
cargo test --workspace
```

---

## License

Kervesh is licensed under the [MIT License](LICENSE).
