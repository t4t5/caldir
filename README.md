# caldir

The "file over app" philosophy for calendars.

```
~/calendar/
  work/
    2025-01-15T0900__standup.ics
    2025-01-15T1400__sprint-planning.ics
  personal/
    2025-01-18__birthday-party.ics
```

## Why?

Your calendar shouldn't live behind proprietary apps or APIs. When it's just files:

**You can see it**
```bash
ls ~/calendar/work/2025-01*
```

**You can search it**
```bash
grep -l "alice" ~/calendar/**/*.ics
```

**You can version it**
```bash
cd ~/calendar && git log
```

**AI can read it**
```
You: "How many meetings did I have last week?"
Claude: *reads files directly* "You had 12 meetings..."
```

## Quick start

Make sure you have [Rust and Cargo](https://rust-lang.org/learn/get-started/) installed.

```bash
# Install the CLI
cargo install --path caldir-cli

# Install the provider for your calendar service
cargo install --path caldir-provider-google   # Google Calendar
cargo install --path caldir-provider-icloud   # Apple iCloud

# Connect and follow the instructions in the CLI:
caldir auth google    # or: caldir auth icloud

# Sync your calendar events
caldir sync

# Your calendar is now in ~/calendar
```

## Viewing events

```bash
caldir events              # Next 3 days
caldir today               # Today's events
caldir week                # This week (through Sunday)
caldir events --from 2025-03-01 --to 2025-03-31  # Custom range
```

## How sync works

caldir uses a git-like push/pull model:

- `caldir pull` -- download remote changes to local
- `caldir push` -- upload local changes to remote (including deletions)
- `caldir sync` -- both, in one command
- `caldir status` -- show pending changes in either direction

## Where things live

caldir touches two places on your system:

- **`~/calendar/`** -- your events, one `.ics` file per event, organized into calendar subdirectories
- **`<config_dir>/caldir/`** -- global config (`config.toml`, auto-created on first run) and provider credentials

`<config_dir>` is `~/.config` on Linux, `~/Library/Application Support` on macOS, and `%APPDATA%` on Windows.

The config file is created with all options commented out -- open it to see what's configurable.

## Standard .ics files

Every event is a standard [RFC 5545](https://tools.ietf.org/html/rfc5545) `.ics` file. You can open them in any calendar app, move them around, or sync them with other tools. caldir is just a directory convention and a sync tool -- there's no lock-in.

## How is it different from pimsync?

- **Human-readable filenames**: pimsync uses UUIDs (`a1b2c3d4.ics`). caldir parses events to generate meaningful names (`2025-01-15T0900__standup.ics`).
- **Native APIs**: pimsync is CalDAV-only. Caldir providers can connect directly to APIs like Google Calendar and Microsoft Graph, so that you can keep using your existing calendar setup!
