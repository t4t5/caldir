# caldir-sync

A tool for syncing cloud calendars to a local directory of `.ics` files.

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

**caldir-sync** is a tool that syncs cloud calendars (Google, Outlook, etc.) to this local format.

## Design Decisions

### User-Provided OAuth Credentials

We don't embed Google Cloud credentials in the app. Users create their own Google Cloud project and provide their own client ID and secret.

This is more friction (~10 minutes of setup), but it means:
- No dependency on any third party
- No "unverified app" warnings (it's your own app)
- No single point of failure if a developer's project gets banned
- True independence — the caldir philosophy is about owning your data

### One-Way Sync (Cloud → Local)

The MVP only syncs from cloud to local. The local directory is a read-only mirror.

Two-way sync requires conflict resolution, which is hard. For LLM use cases (AI reasoning about your calendar), read-only is sufficient. If you want to modify events, use the cloud calendar's UI.

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

## Filename Convention

**Timed events:** `YYYY-MM-DDTHHMM__slug.ics`
- Example: `2025-03-20T1500__client-call.ics`

**All-day events:** `YYYY-MM-DD__slug.ics`
- Example: `2025-03-21__offsite.ics`

The slug is derived from the event title: lowercased, spaces replaced with hyphens, special characters removed.

## Configuration

Config lives at `~/.config/caldir/config.toml`:

```toml
# Where to sync events to
calendar_dir = "~/calendar"

[providers.gcal]
client_id = "your-client-id.apps.googleusercontent.com"
client_secret = "your-client-secret"
```

Tokens (refresh token, access token) are stored separately at `~/.config/caldir/tokens.json`.

## Commands

```bash
# Authenticate with Google Calendar
caldir-sync auth

# Sync events to local directory
caldir-sync sync

# List configured providers
caldir-sync status
```

## Development

```bash
# Check for errors
cargo check

# Run
cargo run -- auth
cargo run -- sync
```
