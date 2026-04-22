---
name: caldir
description: Read, create, edit, and sync calendar events as plaintext .ics files using the caldir CLI. Use whenever the user wants to query, add, modify, or sync calendar events — e.g. "what's on my calendar this week", "add a meeting tomorrow at 3", "cancel my Friday standup", "sync my Google calendar". Requires the `caldir` CLI.
---

# caldir

caldir stores calendars as directories of `.ics` files — one event per file, human-readable filenames. Read and edit them with normal file tools; use the `caldir` CLI to create events and sync with providers.

## Directory layout

Each calendar is a subdirectory of the main caldir directory (by default: `~/caldir/`)

```
~/caldir/
  personal/
    2026-03-20T1500__client-call.ics
    2026-03-21__offsite.ics
  work/
    2026-03-25T0900__dentist.ics
    .caldir/              # local sync state — do not touch
```

## Filename convention

- Timed event: `YYYY-MM-DDTHHMM__slug.ics` (time is local, not UTC)
- All-day event: `YYYY-MM-DD__slug.ics`
- Recurring master: `_recurring__slug.ics` (no date prefix — the master covers all occurrences)
- Slug: lowercased title, hyphens for spaces, alphanumerics only

Examples: `2026-03-20T1500__1-1-with-alice.ics`, `2026-03-21__vacation.ics`, `_recurring__weekly-standup.ics`

## Recurring events

A recurring series is one `_recurring__*.ics` file with an `RRULE` property describing the pattern:

```
RRULE:FREQ=WEEKLY;BYDAY=MO,WE,FR
EXDATE:20260320T150000Z          # occurrences that were cancelled
```

Individual occurrences are *not* separate files. To cancel one occurrence, add its start time to `EXDATE`. To modify one occurrence (e.g. reschedule a single standup), create a second file with the same `UID` plus a `RECURRENCE-ID` pointing at the original start time — that file overrides just that instance.

## See calendar

```bash
caldir today                              # today's events across all calendars
caldir week                               # this week (through Sunday)
caldir events --from 2026-04-01 --to 2026-04-30
caldir events -c work                     # restrict to one calendar by slug
```

Dates are `YYYY-MM-DD`. Pass `--from start` to include all past events.

## Searching events

Just use the filesystem — no CLI needed:

```bash
# Find an event by keyword
grep -l "Alice" ~/caldir/**/*.ics

# Read one
cat ~/caldir/work/2026-03-25T0900__dentist.ics
```

## Creating events

Prefer the CLI — it handles ICS formatting, UIDs, and sync metadata:

```bash
caldir new "Meeting with Alice" --start 2026-03-20T15:00
caldir new "Team standup" --start 2026-03-20T09:00 --duration 30m
caldir new "Vacation" --start 2026-03-25 --end 2026-03-28     # all-day
caldir new "Sprint planning" --start 2026-03-22T10:00 --calendar work
```

Without `--calendar`, the default from `~/.config/caldir/config.toml` is used. Without `--end` / `--duration`, defaults to 1 hour (timed) or 1 day (all-day).

## Editing and deleting

Edit the `.ics` file directly, or delete it with `rm`. Then sync:

```bash
caldir status       # show pending local changes
caldir push         # upload changes to the provider (creates, updates, deletes)
```

## Syncing with providers

```bash
caldir status       # see pending changes in both directions
caldir pull         # download changes from provider
caldir push         # upload local changes to provider
caldir sync         # upload AND download changes to/from provider
```

Only events within ±365 days of today are synced. Conflicts resolve by last-write-wins.

## Invites

```bash
caldir invites                                 # pending invites (next 30 days)
caldir invites --all                           # include already-responded
caldir rsvp ~/caldir/work/2026-03-15T1000__standup.ics accept   # or decline, maybe
caldir rsvp                                    # interactive walkthrough
```

Run `caldir push` after responding to sync the RSVP to the server.

## Tips

- After editing a `.ics` file by hand, run `caldir status` to confirm the change is detected before pushing.
- Timezone is stored per event; `caldir new` uses the system timezone.
- Full docs: https://caldir.org/docs.md
