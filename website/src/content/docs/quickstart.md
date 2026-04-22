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
caldir connect google # Google Calendar

caldir connect icloud # iCloud

caldir connect caldav # CalDAV

caldir connect outlook # Outlook

caldir connect webcal # Public ICS feeds
```

Complete the authentication and your calendar will automatically be fetched.

## Sync your events

Once a calendar has been connected, you can pull its data:

```bash
caldir pull
```

That's it! your ICS files should now appear in your directory:

```
~/caldir/
└── google/
    ├── 2025-03-25T0900__dentist.ics
    └── 2025-03-26T1400__sprint-planning.ics
```
