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

```bash
caldir connect google # Google Cal

caldir connect icloud # iCloud

caldir connect caldav # CalDAV

caldir connect outlook # Outlook

caldir connect webcal # Public ICS feeds
```

Complete the authentication so that your calendars can be fetched.

## Sync your events

```bash
caldir pull
```

Your ICS files should now appear in your directory!

```
~/caldir/
├── icloud/
│   ├── 2025-03-20T1500__client-call.ics
│   └── 2025-03-21__offsite.ics
└── google/
    ├── 2025-03-25T0900__dentist.ics
    └── 2025-03-26T1400__sprint-planning.ics
```
