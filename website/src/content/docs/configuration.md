---
title: Configuration
description: Global and per-calendar configuration
order: 5
---

# Configuration

Global settings are configured in:

- `~/.config/caldir/config.toml` (Linux)
- `~/Library/Application Support/caldir/config.toml` (macOS)
- `%APPDATA%/caldir/config.toml` (Windows)

Example config file:

```toml
# where your data is stored:
calendar_dir = "~/caldir"

# where new events get added:
default_calendar = "personal"

# default reminders for new events:
default_reminders = ["1h", "2h"]
```

By default, the config file has all options commented out.

## Per-calendar config

Each calendar stores its configuration in a local `config.toml`:

```toml
# ~/caldir/personal/.caldir/config.toml
name = "Personal"
color = "#4285f4"

[remote]
provider = "google"
google_account = "me@gmail.com"
google_calendar_id = "primary"
```

Calendars without a `.caldir/config.toml` or without a `[remote]` value are treated as offline calendars (not synced anywhere).
