---
title: Commands
description: CLI command reference
order: 3
---

# Commands

## `caldir connect`

Connect to a [calendar provider](/providers) and fetch its calendars.

```bash
# Google Calendar (hosted OAuth via caldir.org)
caldir connect google
```

You can connect multiple accounts (e.g. personal and work) by running the command multiple times.

## `caldir status`

Show pending changes per calendar, similar to `git status`.

```bash
caldir status

# Show detailed diff
caldir status --verbose

# Status for a specific calendar
caldir status --calendar work
```

## `caldir pull`

Download remote changes to your local caldir directory.

```bash
# Pull events within ±1 year of today
caldir pull

# Pull all events since start
caldir pull --from start

# Pull only a specific calendar
caldir pull --calendar work
```

## `caldir push`

Upload local changes to the remote.

```bash
caldir push

# Push only a specific calendar
caldir push --calendar work
```

Note: if you delete a local `.ics` file and run `push`, the event is also deleted from the remote.


## `caldir sync`

Pull/push in a single command.
```bash
caldir sync
```

## `caldir new`

Create a new event in your local directory.

```bash
# Interactive mode (for humans)
caldir new

# Non-interactive mode (for agents):

# Timed event (defaults to 1 hour)
caldir new "Meeting with Alice" --start 2025-03-20T15:00

# With explicit duration
caldir new "Team standup" --start 2025-03-20T09:00 --duration 30m

# With a location
caldir new "Lunch" --start 2025-03-20T12:00 --location "Café Central"

# With a reminder
caldir new "Sprint planning" --start 2025-03-22T10:00 --reminder 10m

# In a specific calendar
caldir new "Sprint planning" --start 2025-03-22T10:00 --calendar work
```

- If neither `--end` nor `--duration` is specified, new events default to being 1 hour long.
- If `default_reminders` is set in your [global config](/configuration), those reminders are added to new events automatically.

## `caldir events`

View upcoming events. Events that are invites show a colored status indicator: (pending), (accepted), (declined), or (tentative).

```bash
caldir events              # Next 3 days
caldir today               # Today's events
caldir week                # This week (through Sunday)
caldir events --from 2025-03-01 --to 2025-03-31  # Custom range

# Events from one calendar
caldir events --calendar work
```

<details>
<summary>Optional JSON output</summary>

Useful when building scripts, status bar widgets, etc. This is a stable machine-readable interface; consumers should ignore fields they do not understand:

```bash
caldir events --json
```

Returns:

```json
[
  {
    "instance_id": "b2f9@caldir__TZID=Europe/Stockholm:20260814T160000",
    "uid": "b2f9@caldir",
    "calendar": "personal",
    "title": "Friday retro",
    "summary": "Friday retro",
    "all_day": false,
    "start": "2026-08-14T16:00:00+02:00",
    "end": "2026-08-14T16:30:00+02:00",
    "tzid": "Europe/Stockholm",
    "location": "Conference room",
    "description": "Weekly retrospective",
    "status": "confirmed",
    "availability": "busy",
    "visibility": "private",
    "recurrence": null,
    "recurrence_id": "2026-08-14T16:00:00+02:00",
    "organizer": {
      "email": "host@example.com",
      "name": "Host Person"
    },
    "attendees": [
      {
        "email": "me@example.com",
        "name": "Me",
        "status": "accepted"
      }
    ],
    "reminders": [
      { "minutes_before_start": 10 }
    ],
    "url": "https://meet.google.com/abc-defg-hij",
    "attachments": [
      {
        "uri": "https://example.com/agenda.html",
        "params": [
          { "name": "FMTTYPE", "value": "text/html" }
        ]
      }
    ],
    "x_properties": [
      {
        "name": "X-GOOGLE-CONFERENCE",
        "value": "https://meet.google.com/abc-defg-hij",
        "params": []
      }
    ],
    "last_modified": "2026-08-13T10:11:12Z",
    "sequence": 2,
    "rsvp": "accepted",
    "recurring": true
  }
]
```

- `start`/`end` are RFC 3339 timestamps with UTC offset. All-day events use date-only strings, where `end` follows the ICS exclusive-end convention (a one-day event on the 14th has `end` on the 15th).
- `title` is retained as a compatibility alias for `summary`. `all_day`, `tzid`, and `recurring` are convenience fields derived from the event.
- `status` is `confirmed`, `tentative`, or `cancelled`; `availability` is `busy` or `free`; and `visibility` is `public`, `private`, `confidential`, or `null`.
- `recurrence`, when present, contains `rrule`, `exdates`, and `rdates`. The date values use the same date-only or RFC 3339 encoding as `start`. Agenda commands expand recurring series, so an emitted occurrence generally has `recurrence: null` and uses `recurrence_id` to identify its position in the series.
- `organizer` and each attendee contain `email` and optional `name`. An attendee's `status` is `accepted`, `declined`, `tentative`, `needs_action`, or `null`. The top-level `rsvp` is only set when the event is an invite addressed to the calendar's account.
- Each reminder contains a signed `minutes_before_start` integer. `attachments` contain a URI and parameters; `x_properties` contain the unfiltered property name, value, and parameters. Parameters are ordered arrays of `{ "name", "value" }` objects so order and duplicate names can be preserved.
- X-properties are provider-defined. Consumers should ignore names they do not understand and tolerate new JSON fields for forward compatibility.
- `last_modified` is an RFC 3339 UTC timestamp or `null`; `sequence` is the event revision number.

Full JSON may contain attendee addresses, provider event IDs, conference URLs, and HTML alternate descriptions. Do not publish it without reviewing or filtering sensitive fields.

</details>

## `caldir invites`

List pending invites across all calendars (next 30 days). Shows organizer, file path, and current status for each invite.

```bash
caldir invites

# Include already-responded invites (not just pending)
caldir invites --all

# Filter to one calendar
caldir invites --calendar work
```

## `caldir rsvp`

Respond to pending calendar invites. Updates the local ICS file (run `caldir push` afterward to sync your response).

```bash
# Interactive mode (for humans)
caldir rsvp

# Non-interactive mode (for agents)
caldir rsvp ~/caldir/work/2025-03-20T1500__standup.ics accept
caldir rsvp ~/caldir/work/2025-03-20T1500__standup.ics decline
caldir rsvp ~/caldir/work/2025-03-20T1500__standup.ics maybe

```

## `caldir discard`

Discard unpushed local changes, reverting to the remote state.

```bash
caldir discard

# Discard changes in one calendar
caldir discard --calendar work

# Skip confirmation prompt
caldir discard --force
```

## `caldir config`

Show configuration paths and calendar info.

```bash
caldir config

# As JSON
caldir config --json
```

## `caldir update`

Update caldir and all installed providers to the latest version.

```bash
caldir update
```
