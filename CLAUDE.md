# caldir-cli

A CLI for interacting with your local caldir directory and syncing with external calendar providers (Google, Apple, etc.).

## Philosophy

**Plaintext is the ultimate LLM interface.**

Modern AI assistants are surprisingly good at understanding your computer — your dotfiles, your config directories, your shell scripts — because it's all just text files in directories with meaningful names.

Calendars should work the same way. Instead of living behind APIs and OAuth flows, your calendar should be something you can `ls`, `grep`, and reason about locally.

**caldir** is a convention: your calendar is a directory of `.ics` files, one event per file, with human-readable filenames. Each calendar is a subdirectory:

```
~/calendar/
  personal/
    2025-03-20T1500__client-call.ics
    2025-03-21__offsite.ics
  work/
    2025-03-25T0900__dentist.ics
    2025-03-26T1400__sprint-planning.ics
```

**caldir-cli** is the command-line tool for working with caldir directories — syncing with cloud providers, viewing events, and managing your calendar locally.

## Why caldir over vdir/pimsync?

**vdir** is the existing standard for local calendar directories (used by vdirsyncer, pimsync). It specifies:
- Subdirectories = collections (calendars)
- Filenames should be URL-safe and NOT parsed for metadata
- One `.ics` file per event with UID

caldir takes a different approach to filenames:

```
# vdir filenames (opaque IDs)
5a3c9b7e-1234-5678-abcd-ef1234567890.ics

# caldir filenames (human/LLM readable)
2025-03-20T1500__meeting-with-alice_5a3c9b7e.ics
```

**Why human-readable filenames matter:**

1. **`ls` shows your schedule** — No need for a special viewer to see what's on your calendar
2. **grep works** — `ls ~/calendar/work/ | grep 2025-03` shows March events
3. **LLM-friendly** — AI assistants can read your calendar directory and understand it immediately
4. **Sorting works** — Files sort chronologically by default
5. **Tab completion** — Start typing the date to find events

**Why not pimsync/vdirsyncer for Google Calendar?**

| Tool | Google Calendar | Status |
|------|-----------------|--------|
| caldir-cli | Native REST API | Works now |
| vdirsyncer | CalDAV | Works but unmaintained |
| pimsync | CalDAV | Google OAuth not yet implemented |

Google's CalDAV API is a second-class citizen — slower, quirkier, requires separate API enablement. caldir-cli uses Google's native REST API, which is what Google actually supports and maintains.

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
- `push` — Upload local changes to cloud (creates, updates, and deletes)
- `status` — Shows pending changes in both directions

**Sync direction detection** uses timestamp comparison:
- If local file mtime > remote `updated` → push candidate (local was modified)
- If remote `updated` > local file mtime → pull candidate (remote was modified)
- Local-only events with `X-CALDIR-ORIGIN:local` → new events to push
- Remote-only events → new events to pull
- Events in sync state but missing locally → deleted locally, delete from remote on push

**Sync time window**: Only events within ±365 days of today are synced. Events outside this window are left untouched locally (not flagged for deletion just because they weren't fetched from the remote).

**Delete sync**: When you delete a local `.ics` file and run `push`, the event is also deleted from the remote. This is tracked via the sync state file (see below).

### Sync State

Each calendar directory contains a `.caldir-sync` file that tracks which event UIDs have been synced:

```json
{
  "synced_uids": ["abc123", "def456", "ghi789"]
}
```

This is used for **delete detection**: if a UID is in `synced_uids` but has no corresponding local file, the event was deleted locally and should be deleted from the remote on the next `push`.

The sync state is updated automatically after each `pull` or `push` operation. If the file is deleted, the next `pull` will re-download all events and recreate it.

**Safety feature**: If you accidentally delete all local files (empty calendar) and run `push`, caldir-cli will refuse to delete all remote events unless you use `--force`.

### Provider Architecture

The codebase is structured for multiple providers:
- `google` — Google Calendar (OAuth + REST API)
- `ical` — Any iCal URL (read-only, no OAuth)
- `caldav` — Generic CalDAV servers
- `outlook` — Microsoft Graph API

Currently only `google` is implemented.

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
    google.rs    - Google Calendar provider
```

### Key Abstractions

**event.rs** — Provider-neutral event types (`Event`, `Attendee`, `Reminder`, etc.). Providers convert their API responses into these types, and the rest of the codebase works exclusively with them. This keeps provider-specific logic contained.

**diff.rs** — Bidirectional diff computation. Compares remote events against local files and returns `SyncDiff` with separate lists for pull changes (`to_pull_create/update/delete`) and push changes (`to_push_create/update/delete`). Uses timestamp comparison to determine sync direction. Accepts sync state (set of previously synced UIDs) to detect local deletions. Accepts an optional time range to avoid flagging old events for deletion when they fall outside the queried window.

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

The slug is derived from the event title: lowercased, spaces replaced with hyphens, special characters removed. The event ID suffix (8-char hash) ensures uniqueness when multiple events have the same title and time.

## Configuration

Config lives at `~/.config/caldir/config.toml`:

```toml
# Where calendar subdirectories live
calendar_dir = "~/calendar"

# Default calendar for new events (used when --calendar not specified)
default_calendar = "personal"

# Calendar configurations (auto-populated by 'caldir auth')
[calendars.personal]
provider = "google"
account = "me@gmail.com"
# calendar_id is omitted for primary calendar

[calendars.work]
provider = "google"
account = "me@gmail.com"
calendar_id = "work@group.calendar.google.com"

# Provider credentials
[providers.google]
client_id = "your-client-id.apps.googleusercontent.com"
client_secret = "your-client-secret"
```

Running `caldir auth` automatically discovers all calendars for the authenticated account and adds them to `config.toml`.

Tokens are stored separately at `~/.config/caldir/tokens.json`, keyed by provider and account email:

```json
{
  "google": {
    "me@gmail.com": {
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
# Authenticate with Google Calendar (auto-adds calendars to config)
caldir-cli auth

# Create a new local event (uses default_calendar from config)
caldir-cli new "Meeting with Alice" --start 2025-03-20T15:00
caldir-cli new "Team standup" --start 2025-03-20T09:00 --duration 30m
caldir-cli new "Vacation" --start 2025-03-25 --end 2025-03-28  # all-day event

# Create event in a specific calendar
caldir-cli new "Sprint planning" --start 2025-03-22T10:00 --calendar work

# Pull events from all configured calendars
caldir-cli pull

# Push local changes to cloud (including deletions)
caldir-cli push

# Force push even when local calendar is empty (dangerous - will delete all remote events)
caldir-cli push --force

# Show pending changes per calendar (like git status)
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
- `--calendar, -c` — Calendar to create the event in (defaults to `default_calendar` from config)

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
- **icalendar** — Generate and parse .ics files
- **tokio** — Async runtime
- **clap** — CLI argument parsing
- **uuid** — Generate unique event IDs for locally-created events
