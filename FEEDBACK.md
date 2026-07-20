# Feedback on `bases` branch

Both reviewers agree: **keep this branch's architecture** — `SyncBases(HashMap<Id, Option<Box<Event>>>)` unifies the two state mechanisms cleanly, honors "bases are never removed", keeps the diff reviewable, and hashed filenames beat percent-encoding. Don't restore `event-bases` wholesale; port back a small, targeted subset. But the branch is only the infrastructure so far — the two headline benefits of BASES_IDEA.md aren't implemented yet, and there's one real bug.

## Must fix (in priority order)

### 3. Already-in-sync events never get a base

`calendar_diff.rs:39` skips equal pairs, and bases are only recorded for applied changes (`connection.rs`). An upgraded legacy calendar's stable events stay on the `None`/mtime path indefinitely — migration never completes, and `sync_file_mtime` back-dating can never be retired. Record a base for equal pairs that lack one (a small `bases_to_record` collection; `event-bases` had this).

COMMENT: How can we implement this in a clean way? A previous version added synced_events to
CalendarDiff (@caldir-core/src/diff/calendar_diff.rs#L10) but that feels ugly to me.
CalendarDiff is very clean as it is with just outgoing + incoming.

### 4. Every sync rewrites every base file

`SyncBases::save` writes all `Some` values via `EventBases::write_from`, and it runs on both the pull and push apply paths — a one-change sync of a 1,000-event calendar does ~2,000 tempfile+rename cycles, scaling with total history forever (bases are never removed).
