# Kervesh Roadmap

This document outlines upcoming capabilities and engineering milestones for Kervesh.

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
