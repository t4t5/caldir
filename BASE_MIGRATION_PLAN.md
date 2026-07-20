# Event-base migration plan

Goal: one sync-state mechanism — the `bases/` dir — with mtime demoted to a
conflict tiebreak and `known_event_ids` retired.

Constraint: rencal embeds `caldir-core` from crates.io and updates independently
of the CLI. Old and new cores will write the same `.caldir/state/` for months,
possibly concurrently. The cleanup is therefore gated by a state-format version,
not a release number.

## End state (format 2)

- `state/bases/` is the only sync state. A base's presence = "synced before"
  (replaces `known_event_ids.contains`); its content = last agreed state (the
  three-way anchor).
- `Base` = `Snapshot(Event)` (`<id>.ics`) | `LegacyTombstone` (`<id>.tombstone`,
  content = the ID string; `EventInstanceId` already round-trips via
  `From<&str>`). A tombstone means "synced before, content unknown"; only the
  migration creates them, and they upgrade to snapshots as events resync.
- Bases are never removed. Propagated deletes leave the snapshot untouched —
  `known_event_ids`' retention semantics, made content-aware.
- **Corruption invariant:** an unreadable base file (zero-byte or unparseable
  `.ics`, garbled `.tombstone`) is *no base*. Corruption degrades toward
  possible resurrection, never toward delete-propagation.
- Tombstone → snapshot upgrade: atomically write `<id>.ics`, then remove
  `<id>.tombstone` (failable cleanup). If both exist at load, a valid snapshot
  wins (proves a later resync). A corrupt `.ics` beside a tombstone is *no
  base* — the tombstone is likely stale, and falling back to it could
  delete-propagate, violating the invariant.
- mtime's only job is the both-changed tiebreak; `sync_file_mtime` back-dating
  is gone.
- `state/format` holds the format number; cores refuse to sync state newer than
  they support.

### The exhaustive diff table

Guards, evaluated first (order matters; these are the bug-prone part):

1. Local present + absent from remote response + no occurrence in the sync
   window → no-op, regardless of base. Out-of-window absence is
   indistinguishable from deletion.
2. Remote `STATUS:CANCELLED` ≈ absent for delete purposes: both cancelled →
   no-op; remote cancelled + local missing → no-op (don't recreate locally
   deleted events, don't push deletes at already-cancelled ones).

"LWW tiebreak" below = `local_is_newer`: differing `SEQUENCE` when the remote
lacks `LAST-MODIFIED`, else mtime vs `LAST-MODIFIED`.

| base | local | remote | action |
|---|---|---|---|
| none | ✓ | — | push create (never synced) |
| none | — | ✓ | pull create |
| none | ✓ | ✓ | bootstrap: equal → record base; differ → LWW tiebreak, base recorded on convergence |
| tombstone | ✓ | — | pull delete (legacy `known_event_ids` behavior) |
| tombstone | — | ✓ | push delete |
| tombstone | ✓ | ✓ | bootstrap path; real base recorded on convergence |
| tombstone | — | — | no-op |
| snapshot | ✓ | — | `local == base` → pull delete; else modify/delete conflict: keep the edit (push create) or warn — never silently destroy |
| snapshot | — | ✓ | `remote == base` → push delete (covers stale resurrection); else changed since deletion → pull create (resurrect) or warn |
| snapshot | ✓ | ✓ | equal → refresh base if stale; only local changed → push update; only remote changed → pull update; both → LWW tiebreak |
| snapshot | — | — | no-op |

The `snapshot / — / ✓` split is the one behavior change vs. today: reappeared
IDs are currently re-deleted unconditionally; content-awareness lets a genuine
recreation survive.

---

## Phase 1 — this branch + next release (format 1)

All additive. Old cores ignore `bases/` and `state/format`; stale bases only
degrade the new core to the old LWW behavior, never corrupt.

### On this branch

1. **Incremental base writes.** `EventBases::write` currently clears and
   rewrites the whole dir every sync — worst shape for concurrent writers.
   Instead: atomically write upserted bases, unlink removed ones, touch nothing
   on a no-op sync. (Unlinking on delete is transitional, despite the end
   state's "never removed": during format 1, `known_event_ids` carries deletion
   memory and migration converts it to tombstones. Retention begins at
   format 2.)
2. **`state/format` guard.** Write `1` on state creation; backfill on sync-state
   open. Checked only where sync state is opened (diff/pull/push), never on
   read paths — listing/editing ICS files always works; only sync is refused.
   - `> SUPPORTED_FORMAT` → "written by a newer caldir" error
   - unparseable → fail closed, naming the file (guessing format 1 on garbage
     defeats the guard)
   - missing → format 1 (the pre-guard state)
   The guard ships useless and becomes essential: every release without it
   extends the window where format 2 is impossible.
3. **Cleanups:** replace the `<[Result<Event, _>; 1]>::try_from` gymnastics in
   `bases.rs` with an iterator match; inline `event_base_needs_refresh`; fold
   the test-only `CalendarDiff::compute` wrapper into
   `compute_with_event_bases`; import `Event`/`EventInstanceId` instead of
   `crate::`-qualified paths.
4. **Edits win modify/delete conflicts.** If the local event changed while the
   remote event was deleted, push it as a create. If the remote event changed
   while the local event was deleted, pull it as a create. Deletion is the only
   unrecoverable outcome, so the first release that detects these conflicts
   must not silently let it win.
5. **Tests:** pull → hand-edit → push round trip (base == pushed result);
   out-of-window event with a base (the window guard is load-bearing); no-op
   sync leaves `bases/` untouched.

### Release coordination

- Release CLI with bumped `caldir-core`; bump rencal's dependency and ship it
  promptly.

### Keep until format 2

- `known_event_ids` dual-writing — old cores without it re-push everything and
  resurrect deletes; the file costs nothing.
- `sync_file_mtime` back-dating — old cores' direction logic depends on it.
- The no-base LWW path — also the permanent bootstrap path for `caldir connect`
  onto a pre-populated dir; it never fully disappears.

---

## Phase 2 — later release (format 2, breaking)

Trigger: pre-guard cores are effectively extinct — rencal has shipped a
guard-aware core for a comfortable window, ideally with auto-update.

1. **Migrate when sync state is opened** (automatic, idempotent; never on read
   paths — migration locks out old cores and must not be a side effect of
   `caldir list`). The atomic format write is the commit point; every
   intermediate state is valid:
   1. for each `known_event_ids` ID lacking a base, write `bases/<id>.tombstone`
   2. atomically write `2` to `state/format` (tempfile + rename)
   3. leave `known_event_ids` in place — an old core mid-sync may still write
      it; deleting now could lose an ID (lost deletion memory → resurrection)
   A later format-2 open re-imports leftover IDs, then deletes the file — the
   same idempotent pass crash recovery needs. Crash before step 2 → valid
   format 1, migration re-runs. Crash after → format 2 committed; the leftover
   is imported next open. A continuously running pre-guard writer can still
   race the deferred delete — unfixable without a lock; that population is what
   the extinction gate is for. Calendars that never sync never migrate; their
   legacy state stays inert.
2. **Implement the exhaustive diff table** (deletes no longer touch base state).
3. **Delete the legacy model:** `SyncedEventIds` + `known_event_ids` I/O,
   `sync_file_mtime` + the mtime lore comments, the `removed_event_bases`
   plumbing. mtime survives only inside `local_is_newer`.
4. **Tests:**
   - one per diff-table row, including both sub-cases of the snapshot rows
   - migration crash points: interrupt after each step, reopen → valid format 1
     or valid format 2, never mixed
   - ID appended to `known_event_ids` after import → tombstoned by next open
   - corruption (zero-byte `.ics`, garbled `.tombstone`, corrupt `.ics` beside
     a valid tombstone → no base) → never delete-propagation
   - valid `.ics` + `.tombstone` coexisting → snapshot wins
   - format guard: newer / unparseable / missing
5. **Docs:** update `specs/caldir.md`; release notes: format-2 caldirs are
   refused by guard-aware cores, misread by pre-guard (≤0.11.2-era) cores —
   hence the gate.

## Non-goals

- No field-level merge — events stay atomic.
- No lockfile — atomic per-file writes suffice; add an advisory lock only if
  real interleaving bugs appear.
