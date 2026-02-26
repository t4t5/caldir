---
title: Commands
description: CLI command reference
order: 3
---

# Commands

## `caldir connect`

Connect to a cloud calendar provider.

```bash
# Google Calendar (hosted OAuth via caldir.org)
caldir connect google

# Google with your own credentials
caldir connect google --hosted=false

# Apple iCloud
caldir connect icloud
```

This authenticates with the provider, fetches your calendars, and creates a local directory for each one with a `.caldir/config.toml` configuration file.

You can connect multiple accounts by running the command multiple times.

## `caldir pull`

Download remote changes to local.

```bash
caldir pull

# Pull a specific date range
caldir pull --from 2024-01-01 --to 2024-12-31

# Pull all past events
caldir pull --from start

# Pull only one calendar
caldir pull --calendar work
```

## `caldir push`

Upload local changes to the remote, including deletions.

```bash
caldir push

# Push only one calendar
caldir push --calendar work
```

If the local calendar is empty (all files deleted), push will refuse to delete all remote events. Run `caldir pull` to restore them.

## `caldir sync`

Pull then push in one command.

```bash
caldir sync

# Sync a specific date range
caldir sync --from 2024-01-01 --to 2024-12-31

# Sync only one calendar
caldir sync --calendar work
```

## `caldir status`

Show pending changes per calendar, similar to `git status`.

```bash
caldir status

# Status for a specific date range
caldir status --from 2024-01-01 --to 2024-12-31

# Show all events instead of compact counts
caldir status --verbose

# Status for one calendar
caldir status --calendar work
```

## `caldir new`

Create a new local event.

```bash
# Interactive mode (prompts for details)
caldir new

# Or pass arguments directly:

# Timed event (defaults to 1 hour)
caldir new "Meeting with Alice" --start 2025-03-20T15:00

# With explicit duration
caldir new "Team standup" --start 2025-03-20T09:00 --duration 30m

# All-day event
caldir new "Vacation" --start 2025-03-25 --end 2025-03-28

# With a location
caldir new "Lunch" --start 2025-03-20T12:00 --location "Café Central"

# In a specific calendar (defaults to default_calendar from config)
caldir new "Sprint planning" --start 2025-03-22T10:00 --calendar work
```

If neither `--end` nor `--duration` is specified, defaults to 1 hour for timed events or 1 day for all-day events.

## `caldir events`

View upcoming events.

```bash
caldir events              # Next 3 days
caldir today               # Today's events
caldir week                # This week (through Sunday)
caldir events --from 2025-03-01 --to 2025-03-31  # Custom range

# Events from one calendar
caldir events --calendar work
```

## `caldir discard`

Discard unpushed local changes, reverting to the remote state. Locally created events are deleted, local edits are reverted, and locally deleted events are restored.

```bash
caldir discard

# Discard changes in one calendar
caldir discard --calendar work

# Skip confirmation prompt
caldir discard --force
```

## `caldir config`

Show configuration paths and calendar info.

```bash
caldir config
```

## `caldir update`

Update caldir and all installed providers to the latest version.

```bash
caldir update
```
