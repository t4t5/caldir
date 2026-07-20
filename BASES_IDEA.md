## The problem

Sync state today is `known_event_ids` — a flat list of IDs that have ever
synced. It only answers "have I seen this before?", so the diff has to guess at
everything else from file mtimes:

- Deletes are inferred from "ID in the list, file gone".
- Direction is decided by mtime vs remote `LAST-MODIFIED`, which forces
  `sync_file_mtime` back-dating hacks to keep the comparison honest.

The missing piece is the **base**: the last state both sides agreed on.

## The idea

Store, per event, a snapshot of the last synced version under
`.caldir/state/bases/<id>.ics`. That single mechanism replaces the ID list:

- **presence** of a base = "synced before" (what `known_event_ids` did)
- **content** of a base = the three-way merge anchor

With a base, the diff becomes a real three-way comparison instead of a
heuristic:

| base vs local | base vs remote | meaning |
|---|---|---|
| same | same | nothing happened |
| changed | same | local edit → push |
| same | changed | remote edit → pull |
| changed | changed | genuine conflict → tiebreak |

mtime stops being the direction oracle and is demoted to just the tiebreak for
that last row.

## What this buys

1. **Reliable update direction.** base vs local / base vs remote replaces the
   mtime heuristic for deciding push vs pull, so `sync_file_mtime` back-dating
   can eventually be retired.
2. **One state mechanism instead of two.** Presence of a base subsumes
   `known_event_ids`.

## Non-goal: delete conflict resolution

Bases don't change delete semantics. A delete on either side is honoured and
propagated, even if the other side was edited later — deleting is an explicit
user decision, not a conflict to resolve. No edit-preserving "survive as
create", no content-aware resurrection of deleted events.

## Invariants

- **Bases are never removed.** Retaining a base after a delete is what preserves
  deletion memory — the same reason `known_event_ids` kept IDs forever.
- **Deletes win.** Delete decisions use base *presence* only (the old
  `known_event_ids` semantics); base content never blocks or reverses a delete.
- **Corruption never fails the load.** An unreadable base file is skipped,
  degrading that event to the presence-only legacy entry: deletion memory is
  kept, update direction falls back to the mtime heuristic.
- **Out-of-window absence is not deletion.** An event with no occurrence in the
  fetched window is a no-op regardless of base state.
- **Remote `STATUS:CANCELLED` ≈ absent** for delete purposes.
- **Events stay atomic** — no field-level merge, ever.
