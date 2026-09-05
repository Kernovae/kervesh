# Kervesh by Kernovae

**Your hosts. Your keys. Your machine.**

Native Rust SSH/SFTP workspace for Windows and Linux. No browser runtime,
mandatory account, cloud backend, telemetry or installed `ssh` executable.
Implements the v0.1 workstation scope from the
[product requirements](docs/product-requirements.md).

## Run

Requires Rust 1.92 or newer. Use a desktop session with OpenGL support.

```sh
cargo run -p kervesh
# Optimized executable:
cargo build --release -p kervesh
./target/release/kervesh
```

Windows: run `target\release\kervesh.exe` from PowerShell/Explorer. Build with the
MSVC Rust toolchain and Visual Studio C++ build tools.

Linux build dependencies (Debian/Ubuntu; install `libxkbcommon-x11-0` for X11 runtime):

```sh
sudo apt-get install build-essential pkg-config libx11-dev libxkbcommon-dev libwayland-dev libegl1-mesa-dev libgl1-mesa-dev
```

Fedora: `gcc gcc-c++ make pkgconf-pkg-config libX11-devel libxkbcommon-devel
wayland-devel mesa-libEGL-devel mesa-libGL-devel`. SQLite and the D-Bus client
library are bundled. Runtime needs graphics/window libraries and a Secret
Service provider (GNOME Keyring/KWallet) to remember credentials. When the keyring
is unavailable, leave “Save” unchecked for session-only authentication.

## Use

1. Add a host: name, hostname/IP, port, username and authentication method.
2. Save; double-click the host, or use its context menu → Connect.
3. Supply password/passphrase, or select SSH agent. Saved secrets stay in the OS
   keyring; private keys stay in your selected local files.
4. Verify an unknown host's SHA-256 fingerprint with a trusted source before
   selecting **Trust key and connect**. Changed keys fail closed.
5. Open more hosts in independent tabs. SFTP follows the active session.
6. Double-click folders; right-click files for download, rename, permissions,
   delete and copy path. Use Upload or drop a regular file onto the workspace.
7. Inspect CPU, memory, swap, load, mounts, network and system metadata below the
   terminal. Inspector provides details; Pause stops polling for that session.

Terminal defaults to Desktop clipboard behavior: Ctrl+C copies a selection or
interrupts when nothing is selected; Ctrl+V pastes. Traditional mode preserves
Ctrl+C/Ctrl+V control bytes. Both support Ctrl+Shift+C/V. Configurable
Ctrl+Alt+letter sends literal control bytes. Multiline paste requires confirmation
by default; bracketed paste remains supported.

Shift+PageUp/PageDown scrolls history. Drag selects, double-click selects a word,
and triple-click selects a logical line. Shift+drag overrides remote mouse mode.
Ctrl+F searches local scrollback; Ctrl+Click deliberately opens HTTP/HTTPS or OSC 8
links. Absolute paths offer “Reveal in SFTP” when SFTP is available.

Settings → Terminal edits the global default profile, fonts/fallbacks, cursor,
palette, scrollback, clipboard and behavior. Host forms choose a profile; active
sessions can switch profiles without saving that choice to the host. Use “Save
profile to host” explicitly. Terminal palettes are independent of the app theme.
See [Terminal Foundation](docs/terminal-foundation.md) for details and limitations.

Existing settings still control app theme, monitor interval, hidden files,
JSON import/export and trusted keys. Host profiles retain groups, tags, favorites,
timeout, keepalive and optional reconnect.

## Architecture

| Crate | Responsibility |
|---|---|
| `kervesh-core` | Validated profiles, SQLite, OS credential boundary, procfs metrics |
| `kervesh-ssh` | Embedded russh transport, PTY, SFTP, streaming transfers, monitor tasks |
| `kervesh-terminal` | Alacritty emulator, native egui cell rendering, keyboard/mouse |
| `kervesh` | Native eframe app, host/session/file/transfer UI and settings |

One SSH transport per session. PTY, SFTP and monitoring use separate channels.
Bounded command/event queues and separate tasks prevent file/network operations
from running on the UI thread. Transfer buffers stay at 64 KiB per active stream.

## Local data and portable configuration

Default data path uses the OS application-data directory (`Kernovae/Kervesh` on
Windows; `$XDG_DATA_HOME/kervesh` or `~/.local/share/kervesh` on Linux).
`KERVESH_DATA_DIR` overrides the directory for portable/test workspaces.
SQLite uses WAL transactions; host profiles and trusted fingerprints stay local.

Export schema: [docs/configuration.md](docs/configuration.md). Imports append new
profile IDs; they cannot attach imported profiles to existing saved credentials.
Export excludes credentials, fingerprints and history.

## Validate and package

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 scripts/test-loopback.py   # Linux, requires local OpenSSH server tools
python3 scripts/package.py        # after release build; tar/zip and optional deb/rpm
```

Loopback tests generate temporary keys/configuration, bind only `127.0.0.1`, and
clean up their server. They never connect to saved profiles. Native desktop
smoke/benchmark instructions: [docs/validation.md](docs/validation.md).

CI configuration includes Windows/Linux builds and Linux OpenSSH integration.
**Configured CI is not proof that a platform has passed**; see the validation
report for checks actually executed.

## Release scope

See [requirement coverage](docs/coverage.md) for exact implemented scope and gaps.
This is an initial workstation implementation, not a claim that every terminal,
server and distribution has been qualified. Advanced forwarding/bastions,
recursive directory transfers, remote editor, OpenSSH config import, process
viewer and configurable key bindings remain roadmap work. Linux procfs supplies
v0.1 metrics; other remote operating systems retain SSH/SFTP but lack this collector.

Upload replacement uses staged files and a temporary backup because the SFTP v3
library does not expose atomic POSIX replacement. It is recoverable but not
crash-atomic; see [SECURITY.md](SECURITY.md) for interrupted-transfer behavior.
Performance figures are engineering targets until measured on the supported
platform matrix. License: MIT.
