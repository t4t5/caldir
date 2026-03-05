---
title: Sync
description: How bidirectional sync works with push and pull
order: 2
---

# Sync

caldir uses a git-like push/pull model for syncing with cloud calendar providers.

## Push and pull

- `caldir pull` — download remote changes to local
- `caldir push` — upload local changes to remote
- `caldir sync` — both, in one command
- `caldir status` — show pending changes in either direction

## Sync time window

By default, only events within **365 days** of today (past and future) are synced.

You can override the time window with `--from` and `--to` flags:

```bash
# Pull all past events
caldir pull --from start

# Pull a specific range
caldir pull --from 2024-01-01 --to 2024-12-31
```

## Sync state

Each calendar tracks which events have been synced in `.caldir/state/known_event_ids`. This is a plaintext file with one event ID per line, using the RFC 5545 identity:

- `{uid}` for non-recurring events
- `{uid}__{recurrence_id}` for recurring event instances

If you delete a local `.ics` file and run `push`, the event is also deleted from the remote.
