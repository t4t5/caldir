---
title: Quickstart
description: Install caldir and sync your first calendar
order: 1
---

# Quickstart

Linux or macOS:

```bash
curl -sSf https://caldir.org/install.sh | sh
```

Windows:

```powershell
powershell -c "irm https://caldir.org/install.ps1 | iex"
```

This installs the `caldir` CLI and the default [provider plugins](/providers). Prebuilt binaries are
also available on the [releases page](https://github.com/t4t5/caldir/releases).

<details>
<summary>Or install from source</summary>

Make sure you have [Rust and Cargo](https://rust-lang.org/learn/get-started/) installed.

```bash
# Clone the repo:
git clone https://github.com/t4t5/caldir

# Install the CLI:
cd caldir
cargo install --path caldir-cli

# Install the providers you want:
cargo install --path caldir-provider-google
cargo install --path caldir-provider-icloud
cargo install --path caldir-provider-caldav
cargo install --path caldir-provider-outlook
cargo install --path caldir-provider-webcal
```

</details>

<details>
<summary>Install with Nix</summary>

```bash
nix run github:t4t5/caldir -- --help
```

Or add `github:t4t5/caldir` as a flake input and use `packages.${system}.default` (or `overlays.default`) in your NixOS or home-manager config. The package includes all provider binaries.
</details>

## Connect a calendar

Choose a provider where you have calendar data:

```bash
caldir connect google

caldir connect icloud

caldir connect caldav
```

Complete the authentication process and watch your events be pulled into your directory as
ICS files!

```
~/caldir/
└── google/
    ├── 2025-03-25T0900__dentist.ics
    └── 2025-03-26T1400__sprint-planning.ics
```

## Using agents

To explore caldir with tools like Claude Code or Opencode, ask it to install: [SKILL.md](/skill.md)
