# caldir

Caldir is a tool for storing your calendar as a directory of ICS files. It syncs with cloud providers (Google Calendar, iCloud, Outlook, CalDAV…) using git-like pull/push actions.

## Philosophy

Calendars today are often hidden behind APIs and proprietary sync layers, limiting what you can do.

By turning them into plaintext files, you can use tools like grep to search them, or set up advanced workflows using scripts and agents.

It also makes it really easy to migrate your data from one provider to another.

## Architecture

- **`caldir-core`** — pure library. All business logic: event types, calendar discovery, bidirectional sync, ICS round-tripping, the provider subprocess protocol. CLIs and GUIs consume this directly.
- **`caldir-cli`** — thin CLI shell to interact with your caldir. Should never carry sync logic of its own.
- **`caldir-provider-*`** — independent binaries discovered on `PATH` as `caldir-provider-{name}`. Each speaks JSON over stdin/stdout to core, manages its own credentials and tokens under `~/.config/caldir/providers/{name}/`, and ships separately. Adding a provider needs no core change — like git remote helpers.

The `Caldir` struct is the runtime context object. Every CLI command receives a `&Caldir` and threads it down. Production loads it from disk; tests inject in-memory state via the builder.

## The data model in one paragraph

A *calendar* is any non-hidden subdirectory of `calendar_dir`. The directory itself is the source of truth — even an empty directory full of hand-authored `.ics` files is a valid local-only calendar.

If a calendar is connected to a remote, it gets a `.caldir/` (analogous to `.git/`) holding `config.toml` (provider settings) and `state/known_event_ids` (sync state for delete detection).

Calendars come in three flavors: 
- **local-only** (no remote)
- **read-only** (remote, but provider can't push — webcal feeds, view-only shares)
- **writable** (full bidirectional sync with Google Calendar, Outlook etc)

## Sync model

Bidirectional, last-write-wins, no merge: calendar events are atomic.

Direction is decided by comparing local file mtime against the remote `LAST-MODIFIED`.

Deletes are detected by comparing the live file set against `known_event_ids`.

The default sync window in the CLI is ±365 days from today; events outside the window are left alone, never deleted just for falling outside the fetched range.

## Universal event identity

Across every provider, an event is `(uid, recurrence_id)` per RFC 5545. Provider-specific IDs (e.g., Google's internal event ID) live in `X-*` custom properties so they round-trip but don't leak into core's identity model.

## Timezones

Timezone is data, not just display. Events created locally use the system IANA zone (so filenames show local time). All inbound TZIDs are normalized to IANA at the parse boundary — Microsoft providers convert back to Windows names only at the outbound edge.

## Adding a default provider

When shipping a new default provider with caldir, update:

1. workspace `Cargo.toml` — add the crate to `members`
2. `.github/workflows/release.yml` — single source of truth for what ships in tarballs (`install.sh` and `caldir update` discover from there)
3. `website/src/content/docs/providers.md` and `getting-started.md`
4. `.claude/skills/bump/SKILL.md`

## Specs

`specs/caldir.md` is the canonical ICS-format spec. `specs/rfc5545.txt` is the RFC. `specs/vdir.md` is an alternative convention caldir diverges from.

## Development

```bash
just check   # cargo check + clippy across the workspace
just test    # full test suite
```
