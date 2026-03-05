# caldir

The "file over app" philosophy for calendars.

```
~/caldir/
├── home/
│   └── 2025-03-25T0900__dentist.ics
└── work/
    ├── 2025-03-20T1500__client-call.ics
    └── 2025-03-26T1400__sprint-planning.ics
```

## Why?

Calendars already have an open format, `.ics` files, but they're hidden behind APIs and proprietary sync layers. caldir puts your calendar on disk where it's useful:

**You can search it**
```bash
grep -l "alice" ~/caldir/**/*.ics
```

**You can script it**
```bash
# Daily schedule in your terminal
echo "Today:" && ls ~/caldir/*/$(date +%Y-%m-%d)*
```

**Your AI agent can manage it**
```bash
claude "Move my Thursday meetings to Friday"
# Claude reads, renames, and edits the .ics files directly
```

**Your data is portable**
```bash
caldir connect google
mv ~/caldir/outlook/*.ics ~/caldir/google/
```

## Quick start

```bash
# Install caldir
curl -sSf https://caldir.org/install.sh | sh

# Connect and follow the instructions in the CLI:
caldir connect google    # (or "caldir connect icloud", "caldir connect caldav"...)

# Sync your calendar events
caldir sync

# Your calendar is now in ~/caldir
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
caldir events              # View events (3 days forward by default)
caldir today               # Today's events
caldir week                # This week (until end of Sunday)
caldir events --from 2025-03-01 --to 2025-03-31  # Custom range
```

## How sync works

caldir syncs through **providers** — small plugin binaries that talk to calendar services.

Current caldir providers:
- [Google](https://github.com/caldir/caldir/tree/main/caldir-provider-google)
- [iCloud](https://github.com/caldir/caldir/tree/main/caldir-provider-icloud)
- [Outlook](https://github.com/caldir/caldir/tree/main/caldir-provider-outlook)
- [CalDAV](https://github.com/caldir/caldir/tree/main/caldir-provider-caldav)

A provider is just an executable named `caldir-provider-{name}` that speaks JSON over stdin/stdout. Anyone can write one.

- `caldir pull` -- download remote changes to local
- `caldir push` -- upload local changes to remote (including deletions)
- `caldir sync` -- both, in one command
- `caldir status` -- show pending changes in either direction

## Where things live

- **`~/caldir/`** -- your events, one `.ics` file per event, organized into calendar subdirectories
- **`<config_dir>/caldir/`** -- global config (`config.toml`, auto-created on first run) and provider credentials

## Standard .ics files

Every event is a standard [RFC 5545](https://tools.ietf.org/html/rfc5545) `.ics` file. You can open them in any calendar app, move them around, or sync them with other tools. caldir is just a directory convention and a sync tool. There's no lock-in.
