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

This opens your browser for authentication, fetches your calendars, and creates a local directory for each one under `~/calendar/`.

## Sync your events

```bash
# Pull remote events to local
caldir pull

# Your calendar is now in ~/calendar/
ls ~/calendar/
```

After pulling, you'll have a directory structure like:

```
~/calendar/
  personal/
    2025-03-20T1500__client-call.ics
    2025-03-21__offsite.ics
  work/
    2025-03-25T0900__dentist.ics
    2025-03-26T1400__sprint-planning.ics
```

Each event is a standard `.ics` file with a human-readable filename. You can open them in any calendar app, `cat` them, or let your AI assistant read them directly.

## What's next

- Learn about the [push/pull sync model](/docs/sync)
- See all available [commands](/docs/commands)
- Understand the [filename convention](/docs/filename-convention)
