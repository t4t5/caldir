---
title: Sync
description: How bidirectional sync works with push and pull
order: 2
---

# Sync

caldir uses a git-like push/pull model for syncing with cloud calendar providers.

## Push and pull

- `caldir pull` — download remote changes to local
- `caldir push` — upload local changes to remote (including deletions)
- `caldir sync` — both, in one command
- `caldir status` — show pending changes in either direction

## How sync direction is detected

Sync direction is determined by comparing timestamps and sync state:

| Condition | Direction |
|---|---|
| Local file mtime > remote `updated` | Push (local was modified) |
| Remote `updated` > local file mtime | Pull (remote was modified) |
| Local-only event, not in sync state | New event to push |
| Remote-only event, not in sync state | New event to pull |
| In sync state but missing locally | Deleted locally — delete from remote on push |
| In sync state but missing remotely | Deleted remotely — delete locally on pull |

## Sync time window

By default, only events within **365 days** of today (past and future) are synced. Events outside this window are left untouched locally — they're not flagged for deletion just because they weren't fetched from the remote.

You can override the time window with `--from` and `--to` flags:

```bash
# Pull all past events
caldir pull --from start

# Pull a specific range
caldir pull --from 2024-01-01 --to 2024-12-31
```

## Delete sync

When you delete a local `.ics` file and run `push`, the event is also deleted from the remote. This is tracked via the sync state file (`.caldir/state/known_event_ids`).

**Safety feature**: if you accidentally delete all local files (empty calendar) and run `push`, caldir will refuse to delete all remote events unless you pass `--force`.

## Sync state

Each calendar tracks which events have been synced in `.caldir/state/known_event_ids`. This is a plaintext file with one event ID per line, using the RFC 5545 identity:

- `{uid}` for non-recurring events
- `{uid}__{recurrence_id}` for recurring event instances

If the sync state file is deleted, the next `pull` will re-download all events and recreate it.
