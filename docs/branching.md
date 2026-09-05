# Branches and repository governance

## Branch model

| Branch | Purpose | Source / target |
|---|---|---|
| `main` | Default branch; accepted, releasable baseline | Reviewed PR from `dev`, `release/*`, or an urgent `hotfix/*` |
| `dev` | Integration for the next release | Normal PR target |
| `feat/<topic>` | Feature development | Branch from `dev`; PR to `dev` |
| `fix/<topic>` | Bug fix | Branch from `dev`; PR to `dev` |
| `docs/<topic>`, `chore/<topic>` | Documentation or maintenance | Branch from `dev`; PR to `dev` |
| `release/<version>` | Stabilization for a release | Branch from `dev`; PR to `main`; reconcile into `dev` |
| `hotfix/<topic>` | Urgent fix for accepted baseline | Branch from `main`; PR to `main`; reconcile into `dev` |

Only `main` and `dev` are permanent branches. Feature, bugfix, and chore branches are temporary and should be deleted after merging.

Contributors fork the repository, branch from `dev`, and submit pull requests targeting `dev`. Merges from `dev` to `main` use merge commits to maintain shared history.

## Protection policies & rulesets

Three active server-side rulesets govern repository integrity and review standards:

| Ruleset Name | GitHub Ruleset ID | Target Branches | Summary |
|---|---|---|---|
| **Protected branch integrity** | `22332309` | `main`, `dev` | Prevents branch deletion, blocks non-fast-forward pushes, requires passing `Required CI` status check on strict up-to-date branch. No bypass actors. |
| **Main review** | `22332311` | `main` | Requires pull request, 1 code-owner review approval, stale review dismissal on push, and conversation thread resolution. |
| **Dev review** | `22332312` | `dev` | Requires pull request, thread resolution, and passing status checks. |

Reproducible policy snapshots are versioned under `.github/rulesets/`. Server-side configuration is authoritative.

## Quality gates

Every pull request and push to `main` and `dev` must pass the `Required CI` aggregate workflow check. Required CI validates:
- Code formatting (`cargo fmt --all -- --check`)
- Lints (`cargo clippy --locked --workspace --all-targets -- -D warnings`)
- Unit and workspace tests (`cargo test --locked --workspace`)
- Real OpenSSH loopback integration tests on Linux
- Release binary compilation (`cargo build --locked --release -p kervesh`)
- Linux packaging (`.deb`, `.rpm`, archive)
- Matrix testing on Ubuntu, Windows, Debian, and Fedora environments
