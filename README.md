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

```bash
# Install caldir
curl -sSf https://caldir.org/install.sh | sh

# Connect and follow the instructions in the CLI:
caldir auth google    # or: caldir auth icloud

# Sync your calendar events
caldir sync

# Your calendar is now in ~/calendar
```

<details>
<summary>Install from source</summary>

Make sure you have [Rust and Cargo](https://rust-lang.org/learn/get-started/) installed.

```bash
cargo install --path caldir-cli
cargo install --path caldir-provider-google   # Google Calendar
cargo install --path caldir-provider-icloud   # Apple iCloud
```
</details>

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

## Comparison to other tools

|  | **caldir** | **[vdirsyncer]** | **[pimsync]** | **[calendula]** |
|---|---|---|---|---|
| **Filenames** | Human-readable (`2025-01-15T0900__standup.ics`) | UUID-based | UUID-based | UUID-based |
| **Sync model** | Git-like push/pull | Bidirectional pair sync | Bidirectional pair sync | No sync |
| **Google Calendar** | Native REST API | CalDAV | [Not yet](https://whynothugo.nl/journal/2025/03/04/design-for-google-caldav-support-in-pimsync/) | CalDAV |
| **Language** | Rust | Python | Rust | Rust |

**Human-readable filenames** — vdirsyncer, pimsync, and calendula all follow the [vdir spec](https://vdirsyncer.pimutils.org/en/stable/vdir.html), which uses opaque IDs as filenames. caldir generates names like `2025-01-15T0900__standup.ics` so that `ls` shows your schedule, files sort chronologically, and AI assistants can understand your calendar by reading the directory.

**Provider plugins** — vdirsyncer, pimsync, and calendula are all built around CalDAV, which means adding support for providers with non-standard APIs (like Google) is difficult. caldir uses a plugin architecture where providers are separate binaries (`caldir-provider-google`, `caldir-provider-icloud`, etc.), so anyone can add support for a new calendar service without touching the core.

[vdirsyncer]: https://github.com/pimutils/vdirsyncer
[pimsync]: https://git.sr.ht/~whynothugo/pimsync
[calendula]: https://github.com/pimalaya/calendula
