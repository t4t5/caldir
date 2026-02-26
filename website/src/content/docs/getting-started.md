---
title: Getting started
description: Install caldir and sync your first calendar
order: 1
---

# Getting started

## Install

```bash
curl -sSf https://caldir.org/install.sh | sh
```

This installs the `caldir` CLI and the Google Calendar and iCloud provider plugins.

<details>
<summary>Install from source</summary>

Make sure you have [Rust and Cargo](https://rust-lang.org/learn/get-started/) installed.

```bash
cargo install --path caldir-cli
cargo install --path caldir-provider-google   # Google Calendar
cargo install --path caldir-provider-icloud   # Apple iCloud
```

</details>

## Connect a calendar

```bash
# Google Calendar
caldir connect google

# Apple iCloud
caldir connect icloud
```

This opens your browser for authentication, fetches your calendars, and creates a local directory for each one under `~/caldir/`.

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
  personal/
    2025-03-20T1500__client-call.ics
    2025-03-21__offsite.ics
  work/
    2025-03-25T0900__dentist.ics
    2025-03-26T1400__sprint-planning.ics
```
