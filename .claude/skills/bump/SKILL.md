---
name: bump
description: Analyze changes since last release and bump crate versions
user_invocable: true
---

# /bump — Semver version bumping

Analyze git history since the last release and recommend semver bumps for each crate, then apply them.

## Crates

There are 4 crates to consider:

| Crate | Cargo.toml path |
|---|---|
| `caldir-core` | `caldir-core/Cargo.toml` |
| `caldir-cli` | `caldir-cli/Cargo.toml` |
| `caldir-provider-google` | `caldir-provider-google/Cargo.toml` |
| `caldir-provider-icloud` | `caldir-provider-icloud/Cargo.toml` |

`caldir-cli`, `caldir-provider-google`, and `caldir-provider-icloud` all depend on `caldir-core` — their `caldir-core` dependency version pin must always match core's version.

## Steps

### 1. Find the last release anchor

Find the latest `v*` git tag:

```bash
git tag -l "v*" --sort=-v:refname | head -1
```

Show the user which anchor you're using and how many commits are since it. If no tag exists, or the tag looks stale/wrong, ask the user which commit to use as the anchor.

### 2. List commits since the anchor

```bash
git log <anchor>..HEAD --oneline
```

If there are no commits since the anchor, inform the user there's nothing to bump and stop.

### 3. Map changes to crates

For each commit, check which crate directories were modified:

```bash
git diff --name-only <anchor>..HEAD
```

Map changed files to crates by directory prefix (`caldir-core/`, `caldir-cli/`, `caldir-provider-google/`, `caldir-provider-icloud/`). Ignore changes outside these directories (root Cargo.toml, .claude/, etc.).

### 4. Classify changes and recommend bumps

All crates are pre-1.0, so use these conventions:

- **Breaking changes** (public API changes, removed/renamed exports) → **minor** bump (0.x.0, reset patch to 0)
- **New features** (new commands, new fields, new functionality) → **minor** bump (0.x.0, reset patch to 0)
- **Bug fixes / patches** (fixes, refactors, docs, tests) → **patch** bump (0.0.x)

If a crate had **no changes** but `caldir-core` was bumped, it still needs its `caldir-core` dependency pin updated — but its own version only bumps (patch) if the core changes affect it. Use your judgment.

### 5. Present recommendations

Show a table like:

```
Crate                      Current → Proposed   Reason
caldir-core                0.3.0   → 0.4.0      New sync field added (minor)
caldir-cli                 0.3.0   → 0.4.0      New command + core bump (minor)
caldir-provider-google     0.2.1   → 0.2.2      Bug fix (patch)
caldir-provider-icloud     0.1.2   → (no change) Core dep pin updated only
```

Ask the user to confirm or adjust before applying.

### 6. Apply on confirmation

Edit all relevant `Cargo.toml` files:
- Update each crate's `version` field
- Update `caldir-core` dependency pins in cli/provider crates to match core's new version

Then run `cargo check` to validate and update `Cargo.lock`.

Do NOT create a git commit or git tag — the user will handle that themselves.
