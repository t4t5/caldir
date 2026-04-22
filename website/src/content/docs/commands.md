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
```

## `caldir update`

Update caldir and all installed providers to the latest version.

```bash
caldir update
```
