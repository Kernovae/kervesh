# v0.1 requirement coverage

Scope: section 19, confirmed by user. Both supplied source files contain the same
129 numbered requirement IDs. Source documents are preserved unmodified in
`docs/product-requirements.md`. Later roadmap work is not silently claimed as implemented.

| Domain | Implemented | Limits / deferred |
|---|---|---|
| SSH | Profile create/edit/delete/duplicate; authenticated test before saving; host/IP/port/user; passwords; private keys including passphrases; agent; groups/tags/favorites/search/recents; timeout/keepalive; optional reconnect; explicit fingerprint trust and removal; IPv4/IPv6; secret-free JSON export | OpenSSH import, jump hosts and ProxyCommand deferred. Host certificates unsupported. Agent tries up to eight plain-key identities. Reconnect attempts a new session after a previously live session drops; credentials stay in zeroizing session memory. |
| Terminal | Alacritty ANSI/VT engine, UTF-8, indexed/true color, configured scrollback, selection/copy, bracketed paste, resize, alternate screen, mouse, font size, light/dark theme, independent tabs | Custom key bindings/font-family chooser deferred. Font glyph coverage follows bundled egui fonts. Specific vim/nvim/tmux/btop compatibility still needs interactive qualification. |
| SFTP | Session-bound listing/navigation/back/parent/refresh; create files/folders; rename; remove files/empty folders; upload/download regular files; drag/drop upload; overwrite confirmation; metadata; chmod; filter; path copy; hidden files | Sorting currently directory-first/name only. Recursive folder upload/download and integrated remote editor deferred. Modification time displayed as Unix seconds. Symlink navigation supported; symlink transfers not offered. |
| Transfers | Separate bounded queue, async upload/download, progress, throughput, byte counts, cancel, retry, session history, 64 KiB streaming buffers | Upload replacement uses staged backup, not atomic POSIX replacement. Session/process interruption may leave staging/backup files. No persisted/resumable queue. |
| Monitoring | No agent; same SSH transport, separate exec channels; Linux CPU/core deltas, memory/cache/buffers/swap/load, filesystems/devices/mounts/space/inodes, network interface rates, uptime, hostname/OS/kernel/architecture/CPU model/core count/file handles/process count; health rail/inspector; configured interval, pause/resume, reprobe after reconnect | Linux procfs collector only. All metrics share one interval; no per-metric scheduler, metric selection, local threshold alerts or historical charts. Storage device detail comes from mounted filesystems rather than a full block-device inventory. |
| Security | OS keyring boundary, separate non-serializable credentials, strict host trust before auth, changed fingerprint rejection, no mandatory login/cloud/telemetry, secret-free export, OSC52 disabled | Native keyring lifecycle requires desktop/service validation on each platform. No encrypted portable vault. |
| Storage | SQLite WAL transactions, profiles/preferences/layout flags/recents/trust, documented JSON export, atomic validated imports with new credential identities | Arbitrary named workspace layouts and database migrations beyond initial schema deferred. |
| Native/distribution | Pure Rust application with native OpenGL GUI, Linux/Windows platform-specific keyring, tar/zip/deb/rpm packaging tooling, optional Windows installer recipe, CI matrix | This session validates Linux locally. Windows installer/signing and distro CI execution are not claimed without runs. |

## Roadmap

- v0.2: bastion/forwarding, OpenSSH config import, remote editor with conflict/atomic-save handling, improved inspector and threshold warnings.
- Follow-up SFTP/terminal completeness: recursive safe directory transfers, sorting controls, custom fonts/key bindings and broader terminal qualification.
- v0.3: process viewer, multi-exec, macros/snippets, split panes and cloning.
- Engineering: per-metric polling cadence; platform performance baselines; recovery UX for interrupted staged replacements; platform-native package signing.

All section 19 functional areas have working implementations. Full detailed
numbered-requirement coverage is broader than the v0.1 list and remains explicit
in the limitations above. Performance targets are measured separately from
functional completeness.
