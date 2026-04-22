---
title: Quickstart
description: Install caldir and sync your first calendar
order: 1
---

# Quickstart

## Install

```bash
curl -sSf https://caldir.org/install.sh | sh
```

This installs the `caldir` CLI and the default provider plugins.

<details>
<summary>Install from source</summary>

Make sure you have [Rust and Cargo](https://rust-lang.org/learn/get-started/) installed.

```bash
git clone https://github.com/t4t5/caldir
cd caldir
cargo install --path caldir-cli
cargo install --path caldir-provider-google   # Google Calendar
cargo install --path caldir-provider-icloud   # iCloud
cargo install --path caldir-provider-caldav   # Caldav
cargo install --path caldir-provider-outlook  # Outlook
cargo install --path caldir-provider-webcal   # Webcal (ICS feeds)
```

</details>

## Connect a calendar

```bash
# Google Calendar
caldir connect google

# iCloud
caldir connect icloud

# Caldav
caldir connect caldav

# Outlook
# Install provider first (not included in default install)
cargo install caldir-provider-outlook

# Then connect
caldir connect outlook

# Webcal (ICS feed)
caldir connect webcal
```

This opens your browser for authentication (or prompts for a URL for webcal), fetches your calendars, and creates a local directory for each one under `~/caldir/`.

## Sync your events

```bash
# Pull remote events to local
caldir pull

# Your calendar is now in ~/caldir/
ls ~/caldir/
```

After pulling, you'll have a directory structure like:

```
~/caldir/
├── icloud/
│   ├── 2025-03-20T1500__client-call.ics
│   └── 2025-03-21__offsite.ics
└── google/
    ├── 2025-03-25T0900__dentist.ics
    └── 2025-03-26T1400__sprint-planning.ics
```
