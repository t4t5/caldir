# Event-base migration plan

Goal: converge on **one sync-state mechanism** — the `bases/` dir — with mtime demoted
to a conflict tiebreak and `known_event_ids` retired.

The constraint: rencal embeds `caldir-core` from crates.io and updates independently of
the CLI. Old cores and new cores will write the same `.caldir/state/` for months, and may
run **concurrently** on the same dir. So the cleanup is gated by a state-format version,
not by a release number.

## End state (format 2)

- `state/bases/<id>.ics` is the only sync state. Presence = "synced before"
  (replaces `known_event_ids.contains`); content = last agreed state (three-way anchor).
- Bases are **never removed or rewritten on delete**. A propagated delete leaves the
  snapshot in place untouched — same retention semantics `known_event_ids` has today,
  but content-aware (enables the resurrection distinction below).
- `Base` is an enum: `Snapshot(Event)` (a `.ics` file) | `LegacyTombstone` (a
  `.tombstone` file containing the raw ID). `LegacyTombstone` means "synced before,
  content unknown"; it is created **only** by the `known_event_ids` migration, never
  by normal operation, and disappears naturally as events resync.
- A zero-byte or unparseable `.ics` base is **corruption**, not state: treat as no
  base (bootstrap/LWW path). Corruption must never be able to propagate a delete.
- The diff is a single exhaustive match on `(base, local, remote)`.
- mtime's only job is the both-changed tiebreak. No `sync_file_mtime` back-dating.
- `state/format` contains the format number; cores refuse to sync when it exceeds
  what they support.

### The exhaustive diff table

Guards, evaluated before the table (order matters; these are the bug-prone part):

1. Local present + absent from remote response + no occurrence in the sync window →
   **no-op**, regardless of base. Absence out-of-window is indistinguishable from
   deletion.
2. Remote `STATUS:CANCELLED` ≈ absence for delete purposes: both cancelled → no-op;
   remote cancelled + local missing → no-op (never resurrect tombstoned events,
   never push deletes at already-cancelled events).

Then, matching on `(base, local, remote)`:

| base | local | remote | action |
|---|---|---|---|
| none | ✓ | — | push create (never synced) |
| none | — | ✓ | pull create |
| none | ✓ | ✓ | bootstrap: equal → record base; differ → LWW tiebreak, record base on convergence |
| tombstone | ✓ | — | pull delete (legacy `known_event_ids` behavior) |
| tombstone | — | ✓ | push delete |
| tombstone | ✓ | ✓ | bootstrap path, real base recorded on convergence |
| tombstone | — | — | no-op |
| snapshot | ✓ | — | `local == base` → pull delete; `local != base` → modify/delete conflict: keep the edit (push create) or warn — never silently destroy |
| snapshot | — | ✓ | `remote == base` → push delete (also covers stale resurrection of an already-propagated delete); `remote != base` → changed since deletion → pull create (resurrect) or warn |
| snapshot | ✓ | ✓ | three-way: equal → refresh base if stale; only local changed → push update; only remote changed → pull update; both changed → LWW tiebreak (mtime vs `LAST-MODIFIED`, then `SEQUENCE`) |
| snapshot | — | — | no-op, snapshot retained |

The `snapshot / — / ✓` split is the one behavior change vs. today: reappeared IDs are
currently re-deleted unconditionally; content-awareness lets a genuine
recreation/re-invite survive.

---

## Phase 1 — this branch + next release (format 1)

Everything here is additive. Old cores ignore `bases/` and `state/format`; stale bases
degrade the new core to the old LWW behavior (both-changed fallback), never corrupt.

### On this branch, before merge

1. **Incremental base writes.** `EventBases::write` currently deletes every `.ics` in
   `bases/` and rewrites the full map on every sync — including no-op syncs, and it's
   the worst shape for a concurrent old-core writer. Change `record_sync_state` /
   `CalendarState::write` to:
   - atomically write only upserted bases (keep `write_atomic`)
   - unlink only removed bases
   - skip touching disk entirely when ids/bases/removed are all empty
2. **`state/format` guard.** Write `1` on state creation. Checked only where sync
   state is opened (diff/pull/push) — never on read paths, so listing/editing local
   ICS files always works regardless of format; only sync is refused:
   - `> SUPPORTED_FORMAT` → clear "written by a newer caldir" error
   - present but unparseable → fail closed with a clear error naming the file
     (guessing format 1 on garbage defeats the guard's purpose)
   - missing → format 1 (the defined pre-guard state)
   This ships useless and becomes essential: every core released without it extends
   the window where format 2 is impossible.
3. **Cleanups from review:**
   - replace the `<[Result<Event, _>; 1]>::try_from` gymnastics in `bases.rs` load with
     a plain iterator match
   - inline `event_base_needs_refresh` (`base != event`)
   - delete the test-only `CalendarDiff::compute` wrapper; migrate tests to
     `compute_with_event_bases` (or rename it back to `compute`)
   - import `Event` / `EventInstanceId` instead of `crate::`-qualified paths in
     `connection.rs` / `calendar_diff.rs`
4. **Tests to add:** pull → hand-edit → push round trip asserting base == pushed result;
   `(Some(base), Some(local), None)` with an out-of-window event (the wildcard window
   guard is load-bearing); no-op sync leaves `bases/` untouched (mtime of base files
   unchanged).

### Decide (small, this branch or fast-follow)

- **Modify/delete conflicts.** Both both-changed delete branches currently let the
  delete win, silently destroying an edit. Bases make this case detectable for the
  first time. Recommended: resurrect (keep the edited copy, push/pull it as a create)
  or at minimum warn. Deleting is the only unrecoverable outcome.

### Release coordination

- Bump `caldir-core`, release CLI.
- Bump rencal's `caldir-core` dependency and ship rencal promptly — every rencal
  release on a bases-aware, guard-aware core shrinks the population that format 2
  can break. If rencal lacks auto-update, add it before Phase 2; it's what turns
  "old cores are extinct" from a guess into a short window.

### Ongoing during format 1 (do NOT remove yet)

- Keep dual-writing `known_event_ids` — an old core without it re-pushes everything
  as never-synced (duplication) and resurrects deletes. It's an append-only ID file;
  the cost is nothing.
- Keep `sync_file_mtime` back-dating (`calendar/event.rs`) — old cores' direction
  logic depends on `mtime == LAST-MODIFIED` after pull.
- Keep the no-base LWW diff path — it is also the permanent bootstrap path for
  `caldir connect` onto a pre-populated dir, so it never fully disappears; only its
  role shrinks.

---

## Phase 2 — later release (format 2, breaking)

Trigger: pre-guard cores (≤ the last format-1-unaware release) are effectively extinct —
rencal has shipped guard-aware core for a comfortable window, ideally with auto-update.

1. **Migrate when sync state is opened** (automatic, idempotent, no user action —
   but never on read paths: migration bumps the format and locks out old cores, so
   it must not run as a side effect of `caldir list` or similar).
   The atomic `state/format` write is the commit point — every intermediate state
   must be valid:
   1. for each ID in `known_event_ids` with no base file, write
      `bases/<id>.tombstone` containing the raw ID
   2. atomically write `2` to `state/format` (tempfile + rename, like
      `write_atomic` — otherwise it isn't a commit point)
   3. delete `known_event_ids` (optional cleanup; format-2 code must tolerate a
      leftover)
   Crash before step 2 → tombstones are extra files old cores ignore,
   `known_event_ids` intact, still valid format 1; migration re-runs. Crash after →
   format 2 committed, guard-aware cores refuse cleanly, leftover file is inert.
   Cleanup on later opens is a re-run of the same steps: tombstone any leftover ID
   still lacking a base, then delete — idempotent, and absorbs an ID appended by a
   concurrent old core between step 1 and the commit point.
   A calendar that never syncs never migrates — fine, its legacy state is inert
   until sync needs it.
2. **Diff on the unified model** — implement the exhaustive table above, plus:
   - propagated deletes leave the base file untouched (no conversion, no unlink)
   - unparseable/empty `.ics` base → corruption → no base → bootstrap/LWW; never
     delete-propagation
3. **Delete the legacy model:**
   - `SyncedEventIds` module, `known_event_ids` read/write, `add_new_synced_ids`
   - `sync_file_mtime` + call sites and the mtime lore comments
   - `removed_event_bases` plumbing through `CalendarDiff` / `connection.rs`
     (deletes no longer touch base state at all)
   - mtime remains only inside `local_is_newer` for the both-changed tiebreak
4. **Docs:** update `specs/caldir.md` state section; note in release notes that
   format-2 caldirs are refused by guard-aware old cores and misread by pre-guard
   cores (≤0.11.2-era), hence the gate.

## Non-goals

- No merge/field-level conflict resolution — events stay atomic (philosophy).
- No lockfile yet. Incremental atomic per-file writes handle concurrent writers
  well enough; add an advisory lock on `state/` only if real interleaving bugs appear.
