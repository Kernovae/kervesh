# Kervesh Roadmap

This document outlines upcoming capabilities and engineering milestones for Kervesh.

## v0.1.x — Terminal Foundation

Implemented foundation:
- Explicit terminal font families, ordered local font fallbacks, bundled Hack bold,
  coverage regression tests and removal of manual Unicode drawing tables.
- Desktop/Traditional clipboard profiles, smart copy/interrupt, literal controls,
  multiline preview, bracketed paste, semantic/line selection and mouse override.
- Persisted terminal profiles with host bindings and explicit session-to-host save.
- Independent dark/light/custom terminal palettes, cursor shape/blink and native settings.
- Local scrollback search, OSC 8/HTTP links and deliberate SFTP path reveal.
- Optional OSC 7 directory following with host matching and manual-navigation pause.

### v0.1.2 qualification
- Windows native verification, Wayland capture and interactive full-screen application matrix.
- Font geometry across more fonts/scales, broader CJK/complex-script coverage and custom-font bold faces.
- Wrapped plain-URL detection and richer underline decoration variants.

### v0.1.3 extensions
- Optional shell integration installer and command lifecycle metadata, following the documented protocol.
- Regex search, command blocks, arbitrary keybinding editor and advanced font diagnostics.

Implementation and observed validation: [Terminal Foundation](docs/terminal-foundation.md).

## v0.2 — Connectivity & Remote Editing

- **Bastion & Jump Hosts**: SSH ProxyJump and ProxyCommand support for multi-hop bastion topologies.
- **OpenSSH Configuration Import**: Import and sync host profiles directly from `~/.ssh/config`.
- **Integrated Remote Editor**: Lightweight in-workspace editor for remote configuration files with conflict detection and atomic save.
- **Threshold Alerts**: Visual indicators and configurable warning thresholds in the Inspector.
- **Recursive Directory Transfers**: Safe recursive SFTP directory upload and download with collision handling.

## v0.3 — Power User Workflows & Multi-Session

- **Process Viewer**: Live remote process listing with filtering and signal dispatch.
- **Multi-Exec / Snippets**: Send commands to multiple active sessions simultaneously.
- **Split Panes**: Side-by-side terminal panes within a single workspace tab.
- **Configurable Key Bindings**: User-defined shortcuts for terminal and navigation actions.

## Engineering & Platform Baselines

- **Per-Metric Polling Intervals**: Independent cadences for CPU, memory, filesystem, and network telemetry.
- **Platform Performance Qualification**: Extended multi-session benchmarks and memory profiles across Linux and Windows.
- **Native Package Signing**: Authenticode signing for Windows and repository signatures for Linux packages.
