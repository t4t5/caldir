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

# Verbose output (show all events instead of compact counts)
caldir pull --verbose
```

## `caldir push`

Upload local changes to the remote, including deletions.

```bash
caldir push

# Verbose output
caldir push --verbose

# Force push even when local calendar is empty
caldir push --force
```

## `caldir sync`

Pull then push in one command.

```bash
caldir sync
```

## `caldir status`

Show pending changes per calendar, similar to `git status`.

```bash
caldir status

# Status for a specific date range
caldir status --from 2024-01-01 --to 2024-12-31

# Show all events instead of compact counts
caldir status --verbose
```

## `caldir new`

Create a new local event.

```bash
# Timed event (defaults to 1 hour)
caldir new "Meeting with Alice" --start 2025-03-20T15:00

# With explicit duration
caldir new "Team standup" --start 2025-03-20T09:00 --duration 30m

# All-day event
caldir new "Vacation" --start 2025-03-25 --end 2025-03-28

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
```

## `caldir discard`

Discard local changes (revert to remote state).

```bash
caldir discard
```
