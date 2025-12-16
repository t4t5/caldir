# caldir-cli

A CLI for interacting with your local caldir directory and syncing with external calendar providers (Google, Apple, etc.).

## Philosophy

**Plaintext is the ultimate LLM interface.**

Modern AI assistants are surprisingly good at understanding your computer — your dotfiles, your config directories, your shell scripts — because it's all just text files in directories with meaningful names.

Calendars should work the same way. Instead of living behind APIs and OAuth flows, your calendar should be something you can `ls`, `grep`, and reason about locally.

**caldir** is a convention: your calendar is a directory of `.ics` files, one event per file, with human-readable filenames:

```
~/calendar/
  2025-03-20T1500__client-call.ics
  2025-03-21__offsite.ics
  2025-03-25T0900__dentist.ics
```

**caldir-cli** is the command-line tool for working with caldir directories — syncing with cloud providers, viewing events, and managing your calendar locally.

## Design Decisions

### User-Provided OAuth Credentials

We don't embed Google Cloud credentials in the app. Users create their own Google Cloud project and provide their own client ID and secret.

This is more friction (~10 minutes of setup), but it means:
- No dependency on any third party
- No "unverified app" warnings (it's your own app)
- No single point of failure if a developer's project gets banned
- True independence — the caldir philosophy is about owning your data

### Bidirectional Sync

The tool supports bidirectional sync between cloud and local:
- `pull` — Download changes from cloud to local
- `push` — Upload local changes to cloud (creates and updates)
- `status` — Shows pending changes in both directions

**Sync direction detection** uses timestamp comparison:
- If local file mtime > remote `updated` → push candidate (local was modified)
- If remote `updated` > local file mtime → pull candidate (remote was modified)
- Local-only events → new events to push
- Remote-only events → new events to pull

### Filesystem as State

There's no separate state file tracking which events have been synced. Instead:
- Each `.ics` file contains a `UID` field with the cloud provider's event ID
- On sync, we parse all local `.ics` files to build a UID → filepath map
- This is slightly slower but means the filesystem is the single source of truth

### Provider Architecture

The codebase is structured for multiple providers:
- `gcal` — Google Calendar (OAuth + API)
- `ical` — Any iCal URL (read-only, no OAuth)
- `caldav` — Generic CalDAV servers
- `outlook` — Microsoft Graph API

Currently only `gcal` is implemented.

## Module Architecture

```
src/
  main.rs        - CLI entry point and command implementations
  config.rs      - Configuration and token storage
  event.rs       - Provider-neutral event types
  diff.rs        - Pure diff computation between local and remote
  caldir.rs      - Local directory operations (read/write .ics files)
  ics.rs         - ICS format: generation, parsing, formatting
  providers/
    mod.rs
    gcal.rs      - Google Calendar provider
```

### Key Abstractions

**event.rs** — Provider-neutral event types (`Event`, `Attendee`, `Reminder`, etc.). Providers convert their API responses into these types, and the rest of the codebase works exclusively with them. This keeps provider-specific logic contained.

**diff.rs** — Bidirectional diff computation. Compares remote events against local files and returns `SyncDiff` with separate lists for pull changes (`to_pull_create/update/delete`) and push changes (`to_push_create/update`). Uses timestamp comparison to determine sync direction.

**caldir.rs** — The local calendar directory as a first-class abstraction. Reads all `.ics` files into a UID → LocalEvent map (including file modification times for sync direction detection), writes events, deletes events. The filesystem is the source of truth.

**ics.rs** — Everything ICS format. Generates compliant `.ics` files from `Event` structs, parses properties from existing files, formats values for human-readable output (e.g., alarm triggers like "1 day before"). Provider-neutral — no Google-specific code.

## Event Properties

See `specs/caldir.md` for the full ICS format specification with field-by-field documentation.

Events include these properties (when available from the provider):

- **Core**: summary, description, location, start/end time
- **Recurrence**: RRULE, EXDATE, RECURRENCE-ID for recurring events
- **Attendees**: organizer and participants with response status
- **Reminders**: VALARM components with trigger times
- **Availability**: TRANSP (opaque/transparent for busy/free)
- **Meeting data**: conference/video call URLs
- **Sync metadata**: LAST-MODIFIED, SEQUENCE, DTSTAMP
- **Custom properties**: provider-specific fields (e.g., X-GOOGLE-CONFERENCE) preserved for round-tripping
- **Origin tracking**: X-CALDIR-ORIGIN property marks where an event was created

### X-CALDIR-ORIGIN Property

Events created locally via `caldir-cli new` include `X-CALDIR-ORIGIN:local`. This allows the diff logic to distinguish between:
- **Locally-created events** (have `X-CALDIR-ORIGIN:local`) → candidates for pushing to cloud
- **Remotely-deleted events** (no origin marker, but missing from remote) → candidates for local deletion

This keeps all sync state in the `.ics` files themselves, following the "filesystem as state" philosophy.

### Push Flow for New Events

When `push` creates a new event on Google Calendar:

1. Parse local `.ics` file to get the Event
2. Call Google Calendar API to create the event
3. Google returns the created event with:
   - Google-assigned event ID (replaces `local-{uuid}`)
   - Google-added fields (organizer, default reminders, etc.)
4. Write the Google-returned event back to local file:
   - New filename with Google ID suffix
   - All Google-added fields preserved (ORGANIZER, VALARM, etc.)
   - `X-CALDIR-ORIGIN:local` is removed (no longer needed)

This ensures the local file exactly matches the remote state after push, preventing false "modified" status on subsequent syncs.

## Filename Convention

**Timed events:** `YYYY-MM-DDTHHMM__slug_eventid.ics`
- Example: `2025-03-20T1500__client-call_abc12345.ics`

**All-day events:** `YYYY-MM-DD__slug_eventid.ics`
- Example: `2025-03-21__offsite_def67890.ics`

The slug is derived from the event title: lowercased, spaces replaced with hyphens, special characters removed. The event ID suffix (first 8 chars) ensures uniqueness when multiple events have the same title and time.

## Configuration

Config lives at `~/.config/caldir/config.toml`:

```toml
# Where to sync events to
calendar_dir = "~/calendar"

[providers.gcal]
client_id = "your-client-id.apps.googleusercontent.com"
client_secret = "your-client-secret"
```

Tokens are stored separately at `~/.config/caldir/tokens.json`, keyed by provider and account email (discovered during auth):

```json
{
  "gcal": {
    "personal@gmail.com": {
      "access_token": "...",
      "refresh_token": "...",
      "expires_at": "2025-03-20T15:00:00Z"
    },
    "work@company.com": {
      "access_token": "...",
      "refresh_token": "...",
      "expires_at": "2025-03-20T15:00:00Z"
    }
  }
}
```

This supports multiple accounts per provider. Run `caldir-cli auth` multiple times with different Google accounts to connect them all.

## Commands

```bash
# Authenticate with Google Calendar
caldir-cli auth

# Create a new local event
caldir-cli new "Meeting with Alice" --start 2025-03-20T15:00
caldir-cli new "Team standup" --start 2025-03-20T09:00 --duration 30m
caldir-cli new "Vacation" --start 2025-03-25 --end 2025-03-28  # all-day event

# Pull events from cloud to local directory
caldir-cli pull

# Push local changes to cloud
caldir-cli push

# Show pending changes in both directions (like git status)
# Displays "Changes to be pulled" and "Changes to be pushed"
caldir-cli status

# Show which properties changed for each modified event
caldir-cli status --verbose
```

### new command options

- `TITLE` (positional) — Event title
- `--start, -s` — Start date/time (`2025-03-20` for all-day, `2025-03-20T15:00` for timed)
- `--end, -e` — End date/time (mutually exclusive with --duration)
- `--duration, -d` — Duration (`30m`, `1h`, `2h30m`) (mutually exclusive with --end)
- `--description` — Event description
- `--location, -l` — Event location

If neither `--end` nor `--duration` is specified, defaults to 1 hour for timed events or 1 day for all-day events.

## Development

```bash
# Check for errors
cargo check

# Run
cargo run -- auth
cargo run -- pull
```

## Dependencies

- **google-calendar** — Google Calendar API client (handles OAuth, types, requests)
- **icalendar** — Generate .ics files
- **tokio** — Async runtime
- **clap** — CLI argument parsing
- **uuid** — Generate unique event IDs for locally-created events
