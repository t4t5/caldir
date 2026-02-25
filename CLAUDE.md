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
      known_event_ids  # plaintext, one event ID per line
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

**known_event_ids** — Tracks which events have been synced using their RFC 5545 identity: `{uid}` for non-recurring events, or `{uid}__{recurrence_id}` for recurring event instances. This is used for **delete detection**: if an event ID is in `known_event_ids` but has no corresponding local file, the event was deleted locally and should be deleted from the remote on the next `push`.

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
- `app_config.toml` — OAuth client_id/secret (only for self-hosted auth via `--hosted=false`)
- `session/{account}.toml` — Access/refresh tokens per authenticated account (includes `auth_mode` field: `"hosted"` or `"local"`)

The core CLI is completely provider-agnostic — it just passes provider-prefixed config fields (like `google_account`) to the provider binary.

**Current providers**:
- `caldir-provider-google` — Google Calendar (OAuth + REST API)
- `caldir-provider-icloud` — Apple iCloud (CalDAV + app-specific passwords)

**Future providers** (not yet implemented):
- `caldir-provider-outlook` — Microsoft Graph API
- `caldir-provider-caldav` — Generic CalDAV servers

## Module Architecture

```
caldir-core/                   # Core library (used by CLI and future GUI apps)
  src/
    lib.rs              - Module declarations (no re-exports, use full paths)
    error.rs            - CalDirError enum, CalDirResult type alias
    constants.rs        - DEFAULT_SYNC_DAYS
    event.rs            - Provider-neutral event types (Event, Attendee, Reminder, etc.)
    protocol.rs         - CLI-provider communication protocol (Command enum, Request/Response, auth types)
    provider.rs         - Provider subprocess protocol (JSON over stdin/stdout)
    provider_account.rs - ProviderAccount (provider + account identifier for listing calendars)
    remote.rs           - Remote, RemoteConfig (remote calendar operations)
    calendar.rs         - Calendar struct, event CRUD, sync state
    caldir.rs           - Caldir root directory, calendar discovery
    config/
      mod.rs
      global_config.rs   - GlobalConfig (~/.config/caldir/config.toml)
      calendar_config.rs - CalendarConfig (name, color, remote in .caldir/config.toml)
    local/
      mod.rs
      state.rs        - LocalState (.caldir/state/known_event_ids)
      event.rs        - LocalEvent (event + file metadata)
    ics/
      mod.rs
      generate.rs     - ICS file generation (RFC 5545)
      parse.rs        - ICS file parsing
    sync/
      mod.rs
      diff_kind.rs    - DiffKind enum (Create, Update, Delete)
      event_diff.rs   - EventDiff struct
      calendar_diff.rs - CalendarDiff (diff computation + apply)
      batch_diff.rs   - BatchDiff (multiple calendars)

caldir-cli/                    # Thin CLI layer (TUI rendering only)
  src/
    main.rs      - CLI parsing and command dispatch
    render.rs    - Render trait for colored terminal output
    commands/
      mod.rs
      auth.rs    - Authentication flow
      pull.rs    - Pull remote → local
      push.rs    - Push local → remote
      status.rs  - Show pending changes
      new.rs     - Create local events
    utils/       - Spinners and TUI helpers

caldir-provider-google/        # Google Calendar provider (separate crate)
  src/
    main.rs        - JSON protocol handler (reads stdin, writes stdout)
    app_config.rs  - OAuth credentials (~/.config/caldir/providers/google/app_config.toml)
    session.rs     - Token storage and refresh (~/.config/caldir/providers/google/session/)
    commands/      - Command handlers (auth_init, auth_submit, list_calendars, list_events, etc.)
    google_event/  - Conversion between Google API types and caldir_core types
```

### Key Abstractions

**caldir-core** — The main library containing all business logic for calendar sync. Includes provider-neutral event types (`Event`, `Attendee`, `Reminder`, etc.), calendar management (`Calendar`, `Caldir`), bidirectional sync (`CalendarDiff`, `EventDiff`), ICS file handling, and provider protocol. Both the CLI and providers depend on this crate. Future GUI apps (like MagiCal) can use caldir-core directly without any TUI dependencies. Import types using full module paths (e.g., `caldir_core::diff::CalendarDiff`).

**caldir-cli** — Thin CLI layer that provides TUI rendering via the `Render` trait. All business logic lives in caldir-core; the CLI just handles command dispatch and colored terminal output.

**Calendar** — Represents a single calendar directory (`caldir_core::calendar::Calendar`). Loaded via `Calendar::load(path)` which reads the local config from `.caldir/config.toml`. Provides methods for event CRUD operations and sync state management. The `remote()` method returns `Option<Remote>` — calendars without `.caldir/config.toml` are local-only.

**CalendarDiff** — Bidirectional diff for a single calendar (`caldir_core::diff::CalendarDiff`). Created via `CalendarDiff::from_calendar(&cal).await`. Contains `to_push` and `to_pull` vectors of `EventDiff`. Call `apply_push().await` or `apply_pull()` to sync changes.

**Provider** — Provider subprocess protocol (`caldir_core::remote::provider::remote::provider`). Spawns provider binaries, sends JSON requests to stdin, reads JSON responses from stdout. The protocol is simple: `{command, params}` where params are the provider-prefixed fields from config. Commands: `auth_init`, `auth_submit`, `list_calendars`, `list_events`, `create_event`, `update_event`, `delete_event`.

The two-phase auth protocol (`auth_init` + `auth_submit`) decouples auth UI from the provider:
- `auth_init` returns auth requirements (OAuth URL + state for OAuth providers, or form fields for credential-based providers like iCloud/CalDAV)
- The caller handles UI (CLI opens browser + TCP listener; GUI could use webview or native form)
- `auth_submit` receives gathered credentials and completes authentication

**ProviderAccount** — Combines a Provider with an account identifier (`caldir_core::remote::provider_account::remote::providerAccount`). Used to list all calendars for a specific authenticated account via `list_calendars()`.

**CalendarConfig/CalendarState** — Per-calendar state stored in `.caldir/` directory:
- `CalendarConfig` (`caldir_core::config::calendar_config::CalendarConfig`) — Configuration stored in `.caldir/config.toml` (name, color, optional Remote for syncing)
- `CalendarState` (`caldir_core::calendar::state::CalendarState`) — Sync state stored in `.caldir/state/known_event_ids` (tracks synced event IDs for delete detection)

**ics/** — Pure ICS format (RFC 5545) in `caldir_core::ics`. Generates compliant `.ics` files from `Event` structs, parses properties from existing files. Provider-neutral.

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
- **Custom properties**: provider-specific fields (e.g., X-GOOGLE-EVENT-ID, X-GOOGLE-CONFERENCE) preserved for round-tripping

### Push Flow for New Events

When `push` creates a new event on Google Calendar:

1. Parse local `.ics` file to get the Event
2. Call Google Calendar API to create the event
3. Google returns the created event with:
   - Google-assigned event ID (stored as `X-GOOGLE-EVENT-ID` in custom properties)
   - Google-added fields (organizer, default reminders, etc.)
4. Write the Google-returned event back to local file:
   - Filename based on event date/time and title (with collision suffix if needed)
   - All Google-added fields preserved (ORGANIZER, VALARM, X-GOOGLE-EVENT-ID, etc.)
5. Update sync state with the event's RFC 5545 identity (uid + recurrence_id)

This ensures the local file exactly matches the remote state after push, preventing false "modified" status on subsequent syncs.

### Universal Event Identity

Events are identified by their RFC 5545 identity: `(uid, recurrence_id)`. This works across all providers:

- **UID**: The RFC 5545 UID property (e.g., `abc123@google.com`)
- **recurrence_id**: For recurring event instance overrides, identifies which occurrence (e.g., `20250317T100000Z`)

Provider-specific IDs are stored in custom properties:
- **Google**: `X-GOOGLE-EVENT-ID` — Google's internal event ID, used for API calls
- **CalDAV (iCloud)**: Uses the UID directly for API calls

The sync state file tracks `{uid}` or `{uid}__{recurrence_id}` for delete detection.

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

Each calendar stores its configuration in `.caldir/config.toml` (similar to `.git/config`):

```toml
# ~/calendar/personal/.caldir/config.toml
name = "Personal"
color = "#4285f4"

[remote]
provider = "google"
google_account = "me@gmail.com"
google_calendar_id = "primary"

# ~/calendar/work/.caldir/config.toml
name = "Work"
color = "#0b8043"

[remote]
provider = "google"
google_account = "me@gmail.com"
google_calendar_id = "work@group.calendar.google.com"
```

These files are created automatically by `caldir auth google`. The provider returns the config fields to save (name, color, remote settings), so the CLI doesn't need to know about provider-specific field names. Calendars without `.caldir/config.toml` are treated as local-only (not synced).

**Account identifier convention**: Providers with an account concept include a `{provider}_account` field in their remote config (e.g., `google_account`, `icloud_account`). `Remote::account_identifier()` extracts this for grouping calendars by account. Providers without accounts (e.g., plain CalDAV) simply omit the field.

### Provider Credentials

Provider credentials and tokens are managed by each provider in its own directory:

```
~/.config/caldir/providers/google/
  app_config.toml              # OAuth client_id/secret (only for --hosted=false)
  session/
    me@gmail.com.toml          # Access/refresh tokens (auto-refreshed)
```

**Hosted auth (default):** Just run `caldir auth google`. OAuth is handled via caldir.org — no setup needed. Tokens are refreshed through caldir.org when they expire.

**Self-hosted auth:** For users who want to use their own Google Cloud credentials, run `caldir auth google --hosted=false`. This will prompt you to create OAuth credentials in Google Cloud Console and save them as `app_config.toml`. Tokens are refreshed directly with Google.

Both modes will:
1. Open a browser for OAuth
2. Fetch all calendars from your account
3. Create a directory for each calendar with `.caldir/config.toml`

Supports multiple accounts — run auth multiple times with different Google accounts.

## Commands

```bash
# Authenticate with Google Calendar (hosted OAuth via caldir.org)
caldir auth google

# Authenticate with your own Google Cloud credentials
caldir auth google --hosted=false

# Create a new local event (uses default_calendar from config)
caldir new "Meeting with Alice" --start 2025-03-20T15:00
caldir new "Team standup" --start 2025-03-20T09:00 --duration 30m
caldir new "Vacation" --start 2025-03-25 --end 2025-03-28  # all-day event

# Create event in a specific calendar
caldir new "Sprint planning" --start 2025-03-22T10:00 --calendar work

# Pull events from all configured calendars
caldir pull

# Pull events from a specific date range
caldir pull --from 2024-01-01 --to 2024-12-31

# Pull all past events (from the beginning of time)
caldir pull --from start

# Pull with verbose output (show all events instead of compact counts)
caldir pull --verbose

# Push local changes to cloud (including deletions)
caldir push

# Push with verbose output
caldir push --verbose

# Force push even when local calendar is empty (dangerous - will delete all remote events)
caldir push --force

# Show pending changes per calendar (like git status)
caldir status

# Show status for a specific date range
caldir status --from 2024-01-01 --to 2024-12-31

# Show all events (instead of compact counts when >5 events)
caldir status --verbose
```

If neither `--end` nor `--duration` is specified, defaults to 1 hour for timed events or 1 day for all-day events.

## Development

```bash
# Check for errors
just check
```
