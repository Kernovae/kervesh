# GitHub Ruleset Snapshots

This directory contains versioned JSON policy definitions corresponding to the active server-side rulesets on `Kernovae/kervesh`.

## Important

- **Server-side rulesets are authoritative**: GitHub enforces rulesets configured on the remote repository.
- **Config-as-code snapshots**: Files in this directory are reproducible snapshots of those policies.
- **Applying changes**: Modifying these JSON files alone does not alter repository rulesets. Policy updates must be applied via the GitHub API or repository settings by an authenticated administrator.

## Active Rulesets

| Policy File | Ruleset Name | GitHub Ruleset ID | Target Branches | Enforced Rules |
|---|---|---|---|---|
| `integrity.json` | `Protected branch integrity` | `22332309` | `main`, `dev` | Prevent deletion, prevent non-fast-forward pushes, require `Required CI` status check |
| `main.json` | `Main review` | `22332311` | `main` | Pull request required, 1 code-owner approval, stale review dismissal, thread resolution |
| `dev.json` | `Dev review` | `22332312` | `dev` | Pull request required, 0 approvals (during single-maintainer stage), thread resolution |

## Applying Snapshots via GitHub CLI

To apply or update an existing ruleset from a snapshot:

```sh
gh api --method PUT repos/Kernovae/kervesh/rulesets/22332309 --input .github/rulesets/integrity.json
gh api --method PUT repos/Kernovae/kervesh/rulesets/22332311 --input .github/rulesets/main.json
gh api --method PUT repos/Kernovae/kervesh/rulesets/22332312 --input .github/rulesets/dev.json
```
