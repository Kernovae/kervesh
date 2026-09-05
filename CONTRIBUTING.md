# Contributing

Keep protocol, persistence and rendering concerns in their existing crates. New
features must cite the source requirement or an accepted design change. Avoid
adding network services, mandatory accounts, telemetry or a browser runtime.

Branch from `dev` and target `dev` for normal pull requests. Follow
[branching and governance](docs/branching.md) for release/hotfix work.
Use a feature branch. Add regression tests for behavior/security fixes; run
format, Clippy and workspace tests from README. SSH integration fixtures must
bind loopback and use disposable keys/files. Never test against production hosts.

Document behavior changes and update `docs/coverage.md`. Include Windows/Linux
validation actually performed; do not infer platform support from compilation
alone. Commit messages use Conventional Commits. Keep secrets out of commits and
screenshots. Contributions use the repository's MIT license.
