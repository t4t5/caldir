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
2025-03-20T1500__meeting-with-alice.ics
```

**Why human-readable filenames matter:**

1. **`ls` shows your schedule** — No need for a special viewer to see what's on your calendar
2. **grep works** — `ls ~/calendar/work/ | grep 2025-03` shows March events
3. **LLM-friendly** — AI assistants can read your calendar directory and understand it immediately
4. **Sorting works** — Files sort chronologically by default
5. **Tab completion** — Start typing the date to find events

### Bidirectional Sync

The tool supports bidirectional sync between cloud and local:
- `pull` — Download changes from cloud to local
- `push` — Upload local changes to cloud (creates, updates, and deletes)
- `status` — Shows pending changes in both directions

**Sync direction detection** uses timestamp comparison and sync state:
- If local file mtime > remote `updated` → push candidate (local was modified)
- If remote `updated` > local file mtime → pull candidate (remote was modified)
- Local-only events not in sync state → new events to push
- Remote-only events not in sync state → new events to pull
- Events in sync state but missing locally → deleted locally, delete from remote on push
- Events in sync state but missing remotely → deleted remotely, delete locally on pull

**Sync time window**: Only events within ±365 days of today are synced. Events outside this window are left untouched locally (not flagged for deletion just because they weren't fetched from the remote).

**Delete sync**: When you delete a local `.ics` file and run `push`, the event is also deleted from the remote. This is tracked via the sync state file (see below).

### Local State (`.caldir/` directory)

Each calendar has a `.caldir/` directory (similar to `.git/`) for local state and configuration:

```
~/calendar/personal/
  .caldir/
    config.toml    # remote provider configuration
    state/
      synced_uids  # plaintext, one UID per line
  2025-03-20T1500__meeting.ics
  ...
```

**config.toml** — Remote provider configuration for this calendar:
```toml
[remote]
provider = "google"
google_account = "me@gmail.com"
google_calendar_id = "primary"
```

This is created automatically by `caldir auth google`. Like `.git/config`, it contains the "remote" settings for syncing. The config fields (except `provider`) are returned by the provider's `list_calendars` command, so the CLI remains provider-agnostic.

**synced_uids** — Tracks which event UIDs have been synced. This is used for **delete detection**: if a UID is in `synced_uids` but has no corresponding local file, the event was deleted locally and should be deleted from the remote on the next `push`.

The sync state is updated automatically after each `pull` or `push` operation. If deleted, the next `pull` will re-download all events and recreate it.

**Safety feature**: If you accidentally delete all local files (empty calendar) and run `push`, caldir-cli will refuse to delete all remote events unless you use `--force`.

### Provider Plugin Architecture

Providers are separate binaries that communicate with caldir-cli via JSON over stdin/stdout, similar to git's remote helpers (`git-remote-*`). This enables:

- **Permissionless ecosystem** — Anyone can create `caldir-provider-outlook`
- **Language-agnostic** — Providers can be written in any language
- **Independent versioning** — Providers update separately from core
- **Smaller core binary** — Provider-specific deps stay in provider crates
- **Full autonomy** — Providers manage their own credentials, tokens, and refresh logic

**Discovery**: caldir-cli looks for executables named `caldir-provider-{name}` in PATH.

**Provider autonomy**: Each provider manages its own state in `~/.config/caldir/providers/{name}/`. For example, the Google provider stores:
- `credentials.json` — OAuth client_id/secret (user creates via Google Cloud Console)
- `tokens/{account}.json` — Access/refresh tokens per authenticated account

The core CLI is completely provider-agnostic — it just passes provider-prefixed config fields (like `google_account`) to the provider binary.

**Current providers**:
- `caldir-provider-google` — Google Calendar (OAuth + REST API)

**Future providers** (not yet implemented):
- `caldir-provider-google-cloud` — Hosted OAuth (zero-friction auth via `auth.caldir.dev`)
- `caldir-provider-outlook` — Microsoft Graph API
- `caldir-provider-caldav` — Generic CalDAV servers
- `caldir-provider-ical` — Read-only iCal URLs

## Module Architecture

```
caldir-core/                   # Shared types (used by CLI and providers)
  src/
    lib.rs       - Re-exports
    event.rs     - Provider-neutral event types (Event, Attendee, Reminder, etc.)
    calendar.rs  - ProviderCalendar, CalendarWithConfig (returned by list_calendars)
    protocol.rs  - CLI-provider communication protocol (Command enum, Request/Response)

caldir-cli/                    # Core CLI
  src/
    main.rs      - CLI parsing and command dispatch
    commands/
      mod.rs     
      auth.rs    - Authentication flow
      pull.rs    - Pull remote → local
      push.rs    - Push local → remote
      status.rs  - Show pending changes
      new.rs     - Create local events
    config.rs    - Global config (~/.config/caldir/config.toml), GlobalConfig struct
    local.rs     - Per-calendar state: LocalConfig (.caldir/config.toml), LocalState (.caldir/state/)
    calendar.rs  - Calendar struct, event CRUD, sync state updates
    diff/        - Bidirectional diff computation between local and remote
    ics/         - ICS format: generation, parsing (RFC 5545)
    provider.rs  - Provider subprocess protocol (JSON over stdin/stdout)

caldir-provider-google/        # Google Calendar provider (separate crate)
  src/
    main.rs        - JSON protocol handler (reads stdin, writes stdout)
    app_config.rs  - OAuth credentials (~/.config/caldir/providers/google/app_config.toml)
    session.rs     - Token storage and refresh (~/.config/caldir/providers/google/session/)
    commands/      - Command handlers (authenticate, list_calendars, list_events, etc.)
    convert/       - Conversion between Google API types and caldir_core types
```

### Key Abstractions

**caldir-core** — Shared crate containing provider-neutral event types (`Event`, `Attendee`, `Reminder`, `EventTime`, `ParticipationStatus`, etc.), calendar types (`ProviderCalendar`, `CalendarWithConfig`), and protocol types (`Command`, `Request`, `Response`) with JSON serialization. Both the CLI and providers depend on this crate, ensuring type consistency across the protocol boundary. Providers convert their API responses into these types, and the CLI works exclusively with them. `CalendarWithConfig` pairs calendar metadata with the config to save, keeping the CLI provider-agnostic.

**Calendar** — Represents a single calendar directory. Loaded via `Calendar::load(path)` which reads the local config from `.caldir/config.toml`. Provides methods for event CRUD operations and sync state management. The `remote()` method returns `Option<Remote>` — calendars without `.caldir/config.toml` are local-only.

**provider.rs** — Provider subprocess protocol. Spawns provider binaries, sends JSON requests to stdin, reads JSON responses from stdout. The protocol is simple: `{command, params}` where params are the provider-prefixed fields from config. Commands: `authenticate`, `list_calendars`, `list_events`, `create_event`, `update_event`, `delete_event`.

**diff/** — Bidirectional diff computation. Compares remote events against local files and returns lists for pull changes and push changes. Uses timestamp comparison to determine sync direction. Uses sync state (set of previously synced UIDs) to detect local deletions.

**local.rs** — Per-calendar state stored in `.caldir/` directory:
- `LocalConfig` — Remote configuration stored in `.caldir/config.toml` (provider, account, calendar_id)
- `LocalState` — Sync state stored in `.caldir/state/synced_uids` (tracks synced UIDs for delete detection)

**ics/** — Pure ICS format (RFC 5545). Generates compliant `.ics` files from `Event` structs, parses properties from existing files. Provider-neutral.

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

### Push Flow for New Events

When `push` creates a new event on Google Calendar:

1. Parse local `.ics` file to get the Event
2. Call Google Calendar API to create the event
3. Google returns the created event with:
   - Google-assigned event ID (replaces `local-{uuid}`)
   - Google-added fields (organizer, default reminders, etc.)
4. Write the Google-returned event back to local file:
   - Filename based on event date/time and title (with collision suffix if needed)
   - All Google-added fields preserved (ORGANIZER, VALARM, etc.)
5. Update sync state with the new Google-assigned event ID

This ensures the local file exactly matches the remote state after push, preventing false "modified" status on subsequent syncs.

## Filename Convention

**Timed events:** `YYYY-MM-DDTHHMM__slug.ics`
- Example: `2025-03-20T1500__client-call.ics`

**All-day events:** `YYYY-MM-DD__slug.ics`
- Example: `2025-03-21__offsite.ics`

The slug is derived from the event title: lowercased, spaces replaced with hyphens, special characters removed. If multiple events have the same date/time and title, a numeric suffix is added (`-2`, `-3`, etc.) to ensure uniqueness.

## Configuration

### Global Config

Global settings live at `~/.config/caldir/config.toml`:

```toml
# Where calendar subdirectories live
calendar_dir = "~/calendar"

# Default calendar for new events (used when --calendar not specified)
default_calendar = "personal"
```

### Per-Calendar Config

Each calendar stores its remote configuration in `.caldir/config.toml` (similar to `.git/config`):

```toml
# ~/calendar/personal/.caldir/config.toml
[remote]
provider = "google"
google_account = "me@gmail.com"
google_calendar_id = "primary"

# ~/calendar/work/.caldir/config.toml
[remote]
provider = "google"
google_account = "me@gmail.com"
google_calendar_id = "work@group.calendar.google.com"
```

These files are created automatically by `caldir auth google`. The provider returns the config fields to save (via `CalendarWithConfig`), so the CLI doesn't need to know about provider-specific field names. Calendars without `.caldir/config.toml` are treated as local-only (not synced).

### Provider Credentials

Provider credentials and tokens are managed by each provider in its own directory:

```
~/.config/caldir/providers/google/
  app_config.toml              # OAuth client_id/secret
  session/
    me@gmail.com.toml          # Access/refresh tokens (auto-refreshed)
```

To set up Google Calendar, create `~/.config/caldir/providers/google/app_config.toml`:

```toml
client_id = "your-client-id.apps.googleusercontent.com"
client_secret = "your-client-secret"
```

Then run `caldir auth google` to authenticate. This will:
1. Open a browser for OAuth
2. Fetch all calendars from your account
3. Create a directory for each calendar with `.caldir/config.toml`

Supports multiple accounts — run auth multiple times with different Google accounts.

## Commands

```bash
# Authenticate with Google Calendar (auto-creates calendar directories)
caldir-cli auth google

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

If neither `--end` nor `--duration` is specified, defaults to 1 hour for timed events or 1 day for all-day events.

## Development

```bash
# Check for errors
just check
```
