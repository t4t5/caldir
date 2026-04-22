---
title: Quickstart
description: Install caldir and sync your first calendar
order: 1
---

# Quickstart

```bash
curl -sSf https://caldir.org/install.sh | sh
```

This installs the `caldir` CLI and the default [provider plugins](/providers).

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
