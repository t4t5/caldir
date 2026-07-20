## The problem

Sync state today is `known_event_ids` — a flat list of IDs that have ever
synced. It only answers "have I seen this before?", so the diff has to guess at
everything else from file mtimes:

- Deletes are inferred from "ID in the list, file gone".
- Direction is decided by mtime vs remote `LAST-MODIFIED`, which forces
  `sync_file_mtime` back-dating hacks to keep the comparison honest.
- Modify/delete conflicts are invisible. A remote delete beats a local edit
  silently, and vice versa.

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

1. **Real conflict detection.** Local edited + remote deleted is now
   distinguishable from local unchanged + remote deleted. Deletion is the only
   unrecoverable outcome, so an edit should win over a delete rather than be
   silently destroyed.
2. **Content-aware resurrection.** An event deleted locally that reappears
   remotely is currently re-deleted unconditionally. With a base you can tell a
   stale echo (remote == base → push the delete) from a genuine recreation
   (remote != base → pull it back).
3. **One state mechanism instead of two**, and no mtime back-dating lore.

## Invariants

- **Bases are never removed.** Retaining a base after a delete is what preserves
  deletion memory — the same reason `known_event_ids` kept IDs forever.
- **Corruption means "no base", never "no memory of a base".** An unreadable
  snapshot degrades toward possible resurrection, never toward propagating a
  delete.
- **Out-of-window absence is not deletion.** An event with no occurrence in the
  fetched window is a no-op regardless of base state.
- **Remote `STATUS:CANCELLED` ≈ absent** for delete purposes.
- **Events stay atomic** — no field-level merge, ever.
