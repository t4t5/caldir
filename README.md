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

Calendars already have an open format (`.ics` files) but they're hidden behind APIs and proprietary sync layers.

Caldir connects to any provider and puts your calendar data on disk, so that you can:

**Search it**
```bash
grep -l "holiday" ~/caldir/**/*.ics
```

**Script it**
```bash
# Daily schedule in your terminal
echo "Today:" && ls ~/caldir/*/$(date +%Y-%m-%d)*
```

**Manage it with your AI agent**
```bash
claude "Move my Thursday meetings to Friday"
# Claude reads, renames, and edits the .ics files directly
```

**Keep your data portable**
```bash
# Migrate events from Outlook to Google Calendar
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

## Syncing

- `caldir pull` -- download remote changes to local
- `caldir push` -- upload local changes to remote (including deletions)
- `caldir sync` -- both, in one command
- `caldir status` -- show pending changes in either direction

caldir syncs through **providers** — small plugin binaries that talk to calendar services. For example:

- Google ([caldir-provider-google](https://github.com/caldir/caldir/tree/main/caldir-provider-google))
- iCloud ([caldir-provider-icloud](https://github.com/caldir/caldir/tree/main/caldir-provider-icloud))
- Outlook ([caldir-provider-outlook](https://github.com/caldir/caldir/tree/main/caldir-provider-outlook))
- CalDAV ([caldir-provider-caldav](https://github.com/caldir/caldir/tree/main/caldir-provider-caldav))

A provider is just an executable named `caldir-provider-{name}` that speaks JSON over stdin/stdout. Anyone can create one.

## Configuration

**Caldir's global settings** are stored in in your system's config directory:

```toml
# ~/.config/caldir/config.toml (Linux)
# ~/Library/Application Support/caldir/config.toml (macOS)
calendar_dir = "~/caldir"
default_calendar = "personal"
```

**Your calendar-specific settings** are stored in each calendar's directory:

```toml
# ~/caldir/personal/.caldir/config.toml
name = "Personal"
color = "#4285f4"

[remote]
provider = "google"
google_account = "me@gmail.com"
google_calendar_id = "primary"
```

For more details, check out the [full documentation](https://caldir.org/docs).
