# Recurring-event override push corrupts local state

## Symptom

After pushing a single-occurrence override of an Outlook recurring event:

- The master `.ics` file is **deleted from disk**.
- The override file (`2026-05-07T1600__foo-2.ics`) survives but never makes it
  to the server as an exception.
- A **new, unrelated standalone event** appears on the server (and locally as
  `2026-05-07T1600__foo-2-2.ics`) carrying the override's modified fields.
- `caldir status` then reports the master as a pending push-delete, which on
  the next push would wipe the recurring series on the remote.

## Reproduction

1. Create a recurring event in Outlook (daily/weekly).
2. `caldir pull` — local now has master `foo.ics` (RRULE).
3. Edit a single occurrence in renCal (`UpdateEvent` flow at
   `rencal/src-tauri/src/routes/caldir.rs#L618-718`). This produces a local
   override file with the master's UID + `RECURRENCE-ID`.
4. `caldir push`.

## Root cause

Two independent bugs that compound:

### 1. (Primary) Outlook provider doesn't understand exception overrides

`caldir-provider-outlook/src/commands/create_event.rs` blindly POSTs to
`/me/calendars/{id}/events` regardless of whether the event carries a
`recurrence_id`. Microsoft Graph has no "create exception" affordance on that
endpoint — the `recurrence` field there is for *defining* a series, not for
attaching an instance to one. Graph therefore creates a **standalone** event:
new `id`, new `iCalUId`, no `recurrence`, no `originalStart`. The override's
relationship to the master is silently lost on the wire.

### 2. (Secondary) `apply_push` deletes the local file using the *response's* identity

`caldir-core/src/diff/calendar_diff.rs:39-44`:

```rust
DiffKind::Create => {
    let event   = diff.new.as_ref().expect(...);
    let created = remote.create_event(event).await?;
    self.calendar.update_event(&event.uid, &created)?;   // ← bug
    known_ids.insert(created.unique_id());
}
```

`Calendar::update_event(uid, event)` calls
`delete_event(uid, event.recurrence_id.as_ref())`. Here `event` is `created`
— the *response* — whose `recurrence_id` may differ from the original we
sent.

In our scenario:

- We sent `(master_uid, Some(May 7))`.
- Outlook returned `(new_uid, None)` (a fresh standalone, per bug #1).
- `delete_event(master_uid, None)` matches the **master** and removes it.
- `create_event(created)` writes the new standalone (filename collides with
  the surviving override → `-2` suffix).

Bug #2 is what actually nukes the master file. With bug #1 fixed,
`created.recurrence_id` would equal `event.recurrence_id` and the existing
code would behave correctly — but #2 remains a foot-gun for any provider
whose response identity diverges from the request's.

## Fix plan

### Step 1 — Outlook provider: route exception overrides to the right endpoint

In `caldir-provider-outlook/src/commands/create_event.rs`, if
`cmd.event.recurrence_id.is_some()`:

1. Pull `master_id` from the event's `X-OUTLOOK-EVENT-ID` custom property
   (renCal's `resolve_synthetic_instance` already inherits this from the
   master).
2. `GET /me/events/{master_id}/instances?startDateTime={rid - 1d}&endDateTime={rid + 1d}&$select=id,start,originalStart`
   to find the instance whose `originalStart` (or `start`) equals the
   override's `recurrence_id`.
3. `PATCH /me/events/{instance_id}` with the override body produced by
   `to_outlook(event)`.
4. Convert the response via `from_outlook` and return it. The result will
   carry the master's `iCalUId`, an `originalStart` matching the
   `recurrence_id`, and a unique per-instance `id` — preserving the
   master/override relationship across the round-trip.

If the instance lookup returns no match (recurrence already advanced past
that date, etc.), error out cleanly instead of falling back to the standalone
POST — silent fallback is what put us in this mess to begin with.

### Step 2 — caldir-core: delete by the *sent* identity, not the response's

`caldir-core/src/diff/calendar_diff.rs:39-44` should be:

```rust
DiffKind::Create => {
    let event   = diff.new.as_ref().expect(...);
    let created = remote.create_event(event).await?;
    self.calendar.delete_event(&event.uid, event.recurrence_id.as_ref())?;
    self.calendar.create_event(&created)?;
    known_ids.insert(created.unique_id());
}
```

This makes the local cleanup honor the file we *sent* regardless of how the
provider's response is shaped. Defensive, but cheap.

### Step 3 — Tests

In `caldir-provider-outlook`:

- Unit test: given an `Event` with `recurrence_id = Some(...)` and an
  `X-OUTLOOK-EVENT-ID` custom property, the create-event handler issues
  `GET /me/events/{master_id}/instances?...` followed by
  `PATCH /me/events/{instance_id}` — **not** a `POST` to
  `/me/calendars/{id}/events`. Use a recorded HTTP client or a simple
  endpoint capture.

- Regression: a real push round-trip on a daily series leaves the master
  intact and the override visible on the remote with the correct
  `originalStart`.

In `caldir-core`:

- Unit test: when `remote.create_event` returns an event with a different
  `(uid, recurrence_id)` than what was sent, `apply_push` deletes only the
  *sent* file and the master is untouched. (Locks in step 2.)

## Recovery for the corrupted local calendar

The user's outlook calendar is currently in a broken state:

- `foo.ics` (master) — missing.
- `foo-2.ics` — orphan override, refers to a master that exists only on the
  remote.
- `foo-2-2.ics` — orphan standalone, exists locally and remotely with no
  recurrence relationship.
- `known_event_ids` has the master uid but no matching file.

Suggested cleanup before re-pushing:

1. Delete both `foo-2.ics` and `foo-2-2.ics` locally.
2. Manually delete the orphan standalone "Foo 2" event from the Outlook web
   UI (if the previous push reached the server).
3. `caldir pull` — re-fetches the master from Outlook, repopulating
   `foo.ics`. `known_event_ids` self-heals.
4. Redo the override edit in renCal and push (with the fix applied).

## Cross-provider note

The Google provider may or may not exhibit bug #1 — needs a separate look at
`caldir-provider-google/src/commands/create_event.rs`. Google's API is more
lenient (events can be created with `recurringEventId` + `originalStartTime`
to attach as exceptions), but we shouldn't assume it works without checking.
