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
- `push` — Upload local changes to cloud (updates only)
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

Events include these properties (when available from the provider):

- **Core**: summary, description, location, start/end time
- **Recurrence**: RRULE, EXDATE, RECURRENCE-ID for recurring events
- **Attendees**: organizer and participants with response status
- **Reminders**: VALARM components with trigger times
- **Availability**: TRANSP (opaque/transparent for busy/free)
- **Meeting data**: conference/video call URLs
- **Sync metadata**: LAST-MODIFIED, SEQUENCE, DTSTAMP
- **Custom properties**: provider-specific fields (e.g., X-GOOGLE-CONFERENCE) preserved for round-tripping

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

# Pull events from cloud to local directory
caldir-cli pull

# Push local changes to cloud (updates only, not new events)
caldir-cli push

# Show pending changes in both directions (like git status)
# Displays "Changes to be pulled" and "Changes to be pushed"
caldir-cli status

# Show which properties changed for each modified event
caldir-cli status --verbose
```

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
