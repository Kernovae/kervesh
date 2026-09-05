# Validation report — 2026-09-05

Local machine: Fedora 44 KDE, x86_64, Rust 1.97.1. App uses eframe OpenGL.
All network checks use disposable loopback servers and generated test keys.
No production hosts, saved user profiles or third-party write APIs were used.

## Executed functional checks

- `cargo fmt --all -- --check`.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`.
- `cargo test --locked --workspace`: 17 tests; 2 OpenSSH fixture tests excluded from the default run and executed separately below.
- `python3 scripts/test-loopback.py`: 2 integration tests pass against actual OpenSSH. Encrypted Ed25519 key auth, explicit trust, changed-key rejection, PTY shell, staged streaming upload/download, overwrite refusal/replacement, cancellation, procfs monitoring, independent sessions and a 6 MiB simultaneous terminal input/output stress case.
- Password-auth test uses an embedded disposable russh server. It proves rejected/changed host keys receive zero password-auth attempts, valid password works and invalid password fails.
- Domain tests cover SQLite reopen/delete, duplicate identities, atomic validated imports, no secret fields/exported trust, counter resets/guest CPU accounting and invalid settings.
- Terminal tests cover cursor overwrite, alternate-screen restoration, split UTF-8, resize, bracketed-paste injection and desktop clipboard/control-key translation.
- Native UI headless frames exercise empty workspace and host form. Native Linux/X11 window launched and screenshot visually inspected (`artifacts/workspace-dark.png`).
- `cargo build --locked --release -p kervesh`: optimized native binary.
- Python packaging scripts syntax-checked and exercised for tarball and `.deb`; package metadata inspected with `dpkg-deb --info`.

## Performance

Reproduce with `python3 scripts/benchmark-native.py` after release build.
Requires Linux desktop, X11/XWayland, `/proc` and `xprop`. Script launches an empty
workspace in a temporary data directory, measures time until a matching native
window appears, waits 10 seconds for startup/driver work to settle, samples CPU
for 5 seconds and records RSS. It terminates its own process afterward.

Latest numerical output lives in `artifacts/benchmark-empty.json`. Observed:

| Metric | Observation |
|---|---:|
| Window mapped | 0.250 s on final run (0.19–0.25 s observed) |
| Empty-workspace RSS | 86.34 MiB on final run (86–89 MiB observed) |
| Settled idle CPU | 0.0% of one CPU during 5 s sample |
| Optimized executable | 17.77 MiB |

An earlier sample after only 2 seconds settling measured 5.6% CPU; tracing showed
five UI updates across four seconds, not an application repaint loop. With a
10-second settling period, measured idle CPU reached zero. Results are local
observations, not cross-platform guarantees. RSS includes native window/graphics
libraries and exceeds the aspirational <50 MB empty-workspace target.

For one/five-session measurements, use disposable local hosts, wait for metrics
and shaders to settle, then sample `/proc/<pid>/status` and CPU ticks over the same
interval. Record renderer/driver, resolution, font, scrollback, polling interval,
workload and release binary hash alongside measurements. Twenty-session stress,
file sizes beyond fixture coverage and broad terminal-app qualification remain
release qualification work, not silently inferred from unit tests.

## Platform and artifact limits

- Fedora 44 native launch and loopback behavior verified locally.
- Windows MSVC, Ubuntu, Debian and Fedora CI jobs are configured but were **not run
  remotely** in this session. No Windows GUI validation or signed installer claim.
- `.deb` generated locally is a structural packaging check of a Fedora-built
  binary. Its minimum glibc dependency is derived from imported ELF symbols; it
  must not be presented as a binary validated on older Debian/Ubuntu systems.
  The local artifact requires glibc >= 2.43.
  Build packages on each supported baseline before distributing.
- `.rpm` recipe and Windows ZIP/Inno Setup recipe provided; RPM builder and
  Windows packaging are not available in this local validation environment.
- OS keyring integration is implemented using native backends. Interactive
  unlock/save/delete behavior still requires validation per desktop/Windows
  environment; no existing user secrets were inspected during testing.
- No publish, install into the system, push, release signing or merge to main.

## Bugs caught by integration

1. OpenSSH emits `WindowAdjusted` before channel success. Setup now handles this
   legal event ordering.
2. Native toolkit consumes Ctrl+C/X/V as clipboard events. Terminal converts
   unshifted control combinations back to terminal bytes.
3. Large input originally blocked incoming PTY output. Independent write task
   fixes the deadlock; 6 MiB duplex test proves progress.
