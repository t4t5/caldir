---
title: Configuration
description: Global and per-calendar configuration
order: 4
---

# Configuration

## Global config

Global settings live at `~/.config/caldir/config.toml` (Linux), `~/Library/Application Support/caldir/config.toml` (macOS), or `%APPDATA%/caldir/config.toml` (Windows).

```toml
# Where calendar subdirectories live
calendar_dir = "~/calendar"

# Default calendar for new events (used when --calendar not specified)
default_calendar = "personal"
```

The config file is created with all options commented out on first run — open it to see what's configurable.

## Per-calendar config

Each calendar stores its configuration in `.caldir/config.toml`:

```toml
# ~/calendar/personal/.caldir/config.toml
name = "Personal"
color = "#4285f4"

[remote]
provider = "google"
google_account = "me@gmail.com"
google_calendar_id = "primary"
```

These files are created automatically by `caldir connect`. The provider returns the config fields to save (name, color, remote settings), so the CLI doesn't need to know about provider-specific fields.

Calendars without `.caldir/config.toml` are treated as local-only (not synced).

## Local state directory

Each calendar has a `.caldir/` directory (similar to `.git/`) for configuration and sync state:

```
~/calendar/personal/
  .caldir/
    config.toml           # Remote provider configuration
    state/
      known_event_ids     # Plaintext, one event ID per line
  2025-03-20T1500__meeting.ics
  ...
```

The `known_event_ids` file tracks which events have been synced. This is used for delete detection: if an event ID is in this file but has no corresponding local file, the event was deleted locally and should be deleted from the remote on the next `push`.

## Provider credentials

Provider credentials and tokens are managed by each provider in its own directory:

```
~/.config/caldir/providers/google/
  app_config.toml              # OAuth client_id/secret (only for --hosted=false)
  session/
    me@gmail.com.toml          # Access/refresh tokens (auto-refreshed)
```

Tokens are refreshed automatically when they expire.
