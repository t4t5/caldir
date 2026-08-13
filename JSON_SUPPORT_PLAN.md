# Plan: `--json` output via a view layer

## Goal

Give machine consumers (status bar widgets, waybar/quickshell modules, scripts,
rencal helpers) a stable, parseable way to read caldir output. Today the only
listing surface is human text with ANSI codes, so every consumer would have to
scrape day headers like `"Fri Aug 14"` back into dates. A `--json` flag turns
the CLI into the machine API for caldir.

Context: requested in [rencal#61](https://github.com/t4t5/rencal/issues/61),
first attempted in [PR #28](https://github.com/t4t5/caldir/pull/28). We build
this ourselves; see "Relationship to PR #28" at the bottom for what carries
over.

## Direction (settled in PR #28 review)

**Each command produces one typed result; rendering is dispatched one layer
above.** No `if json { … } else { … }` inside command bodies, and no JSON-only
DTO structs mirroring fields next to a separate text path. The
[`json-output` branch](https://github.com/t4t5/caldir/compare/json-output) is
the proof of concept:

- `commands::config::run()` returns a `ConfigView` instead of printing.
- `output.rs` defines `TextRender` (`to_text()`), `Output: TextRender` with a
  blanket `to_json()` impl for `T: Serialize + TextRender`, an `OutputFormat`
  enum, and `emit(&view, format)`.
- `main.rs` owns the global `--json` flag and calls `emit`.

Commands stay simple, and every future command gets `--json` by returning a
view — no extra plumbing per command.

**The root blocker is that `Serialize` on `Event` means "encode as ICS".**
`event.rs` hard-wires serde to the provider wire format (the comment says it:
"Wire format for events is ICS, not JSON"). That's what forced PR #28 into
mirror structs. Fix it at the source: move ICS encoding into an `Ics<Event>`
newtype used only at the RPC boundary, and free `Event` from serde entirely.

Deliberately *not* doing: a structured `#[derive(Serialize)]` on `Event`
itself. That would make core's internal field layout a de-facto public schema
— any internal refactor would silently change the CLI's wire format. The JSON
contract lives in the CLI views, which project from core types. (The `Ics`
newtype leaves the door open if core ever wants a structured serde form.)

## What both renderers must respect

- **Project forward from core types.** `to_text()` and `Serialize` each map
  view → output. Never parse formatted strings back into types (PR #28's
  `AgendaEventTime::to_event_time()` round-trip with `expect()`s).
- **Text output stays byte-identical.** The existing render code moves, it
  doesn't get rewritten.
- **JSON is a query result, not a display projection.** The text view repeats
  a multi-day event under every day it spans; the JSON must not — consumers
  that count, dedupe, or re-group would have to undo it. Flat, sorted
  occurrences. Day-grouping is a text-renderer concern only.

## The JSON contract (events / today / week)

`caldir events --json` (and `today`, `week`) prints one line of compact JSON:
a flat array of expanded occurrences, sorted like the text view (by day,
all-day first, then start). No ANSI codes. Empty range prints `[]`.

```json
[
  {
    "instance_id": "b2f9…@caldir__20260814T160000Z",
    "uid": "b2f9…@caldir",
    "calendar": "personal",
    "title": "Friday retro",
    "all_day": false,
    "start": "2026-08-14T16:00:00+02:00",
    "end": "2026-08-14T16:30:00+02:00",
    "tzid": "Europe/Stockholm",
    "location": null,
    "description": null,
    "status": "confirmed",
    "rsvp": "accepted",
    "recurring": true
  }
]
```

Field decisions:

- **`instance_id`** — `event_instance_id().to_string()`: the existing stable
  `{uid}__{recurrence_id}` format (round-trippable, usable for dedupe).
- **`start` / `end`** — *resolved* RFC3339 instants with UTC offset, per
  `EventTime` variant: `DateTimeZoned` in its own zone (`tzid` set),
  `DateTimeUtc` as `…Z`, `DateTimeFloating` in the machine's local zone
  (`tzid` null). Consumers can `Date.parse` directly — no tagged enum to
  decode. `end` is null when the event has none.
- **All-day events** — `all_day: true` with date-only strings
  (`"start": "2026-08-14"`). `end` keeps the ICS exclusive-DTEND convention
  (a one-day event on the 14th has `end: "2026-08-15"`) — documented, not
  converted, so it matches the files on disk.
- **`rsvp`** — `accepted` / `declined` / `tentative` / `needs_action`, only
  non-null when the event is an invite addressed to the calendar account's
  email (same `is_invite_for` + `attendee_status` logic as the text view).
  Wire values are spelled here, not borrowed from `Display`.
- **`recurring`** — `recurrence_id.is_some()` (expanded instances carry it).
- **`status`** — `confirmed` / `tentative` (`cancelled` is filtered by
  `is_visible`, same as text).
- **snake_case keys, compact one-line output** — `jq` for humans; a whole
  snapshot on one line makes a future `--watch` plain NDJSON.
- Not in v1: calendar name/color (belongs in a future
  `caldir calendars --json`), attendees/organizer/reminders (grow on demand),
  source file path.

Additive-stable from day one: consumers must tolerate new fields; existing
fields never change meaning. Document on caldir.org.

## Implementation

Four commits/PRs, each independently landable.

### 1. caldir-core: `Ics<Event>` at the RPC boundary

- New `rpc/ics.rs`: `pub struct Ics<T>(pub T)` with `Serialize` (via
  `to_ics_string`) and `Deserialize` (via `from_single_ics_str`) implemented
  for `Ics<Event>`, plus `From<Event>` and `into_inner()`. Export from lib.
- Delete the `Serialize`/`Deserialize` impls on `Event` (`event.rs:277-290`).
  `Event` ends with no serde impls.
- RPC types: `CreateEvent { event: Ics<Event> }`, same for `UpdateEvent`;
  `ListEvents::Response = Vec<Ics<Event>>`. (`Response<T>` serializes `data`
  directly, so the bare `Vec` alias needs the newtype — `#[serde(with)]`
  can't attach to it.)
- Providers (google, icloud, outlook, caldav, webcal): mechanical
  wrap/unwrap at the handler edges, ~10 sites.

**Wire bytes are identical** — still ICS strings inside the JSON RPC — so no
protocol version concerns; existing round-trip tests prove it. Breaking for
core's *Rust* API only: core `0.13.0` → `0.14.0`, providers bumped together
as usual.

### 2. caldir-cli: output layer + `ConfigView`

Adopt the `json-output` branch (it's written; `capture`/`test_utils.rs`
removal is safe — config.rs was its only user). Three amendments:

- `emit` prints **compact** JSON (`serde_json::to_string`), not pretty —
  matches the contract above.
- Guard unsupported commands: `Commands::supports_json()` and a single
  `bail!("--json is not yet supported for this command")` in `main` before
  dispatch. Silently ignoring the flag would be worse.
- Commands that stream progress or prompt (`sync`, `pull`, `push`,
  `connect`, …) stay unsupported — they don't produce one value. NDJSON
  progress events are a separate future design.

### 3. caldir-cli: `AgendaView` for events / today / week

New `views/agenda.rs`:

```rust
pub struct AgendaView {
    time_format: TimeFormat,          // for text rendering; not serialized
    pub entries: Vec<AgendaEntry>,    // flat, sorted, one per occurrence
}

pub struct AgendaEntry {
    pub calendar: Option<String>,
    pub rsvp: Option<ParticipationStatus>,  // resolved at collect time
    pub event: Event,
}

impl AgendaView {
    pub fn collect(caldir, calendars, from, to) -> Result<Self>
}
```

- `collect` is the loop that opens `render_events_in_range` today (expand →
  `is_visible` → sort), minus the `display_days` duplication, plus rsvp
  resolution (`remote_email` is consumed here and dropped).
- `impl TextRender for AgendaView` in `render/events_in_range.rs`: the
  existing day-splitting (`display_days`), grouping, `format_date_label`,
  `format_event_line` — moved, not rewritten. `format_event_line` takes
  `TimeFormat` instead of `&Caldir` (it only reads `time_format()`).
  Empty view renders the dimmed "No events found". Output byte-identical;
  `display_days` and its tests stay put.
- `impl Serialize for AgendaEntry` (hand-written, ~40 lines): the contract
  projection — instance id, resolved times, `all_day`, wire-spelled enums.
  `AgendaView` serializes as the bare array. This is the *only* place the
  JSON shape exists.
- `events.rs` / `today.rs` / `week.rs`: `run()` returns
  `Result<AgendaView>`; range and calendar resolution untouched. `main.rs`
  arms become `emit(&commands::today::run(…)?, output_format)`.

### 4. Docs & release

- `--help` text for the global flag.
- caldir.org: "Machine-readable output" section — the contract example, the
  additive-stability promise, the multi-day counting difference vs text
  (once in JSON, N days in text), the exclusive-DTEND note.
- caldir-cli `0.11.x` → `0.12.0`.

## Testing

- **rpc (commit 1)**: `Ics<Event>` round-trip test; existing protocol tests
  pass unchanged (same bytes).
- **ConfigView (commit 2)**: text + JSON tests from the branch.
- **AgendaView (commit 3)**: JSON snapshots via `serde_json::to_value` +
  `pretty_assertions` (already dev-deps): zoned, UTC, floating, multi-day
  all-day (exclusive end), accepted invite, recurring instance, empty → `[]`.
- **Text safety**: existing render tests unchanged; diff `caldir events` /
  `today` / `week` output before/after.
- **Manual**: `caldir events --json | jq length` vs hand-counted text output.

## Relationship to PR #28

Commit 2 builds directly on the `json-output` PoC branch. From the PR itself
there's little to reuse: its `agenda_view.rs` carries the string round-trip
and day-grouped wire shape, and its text path is a rewrite — commit 3
rebuilds that piece from scratch on top of the `Ics<Event>` foundation. The
PR's contribution is the flag UX and validating the direction; close it with
thanks once this lands.

## Out of scope (follow-ups)

- **`--watch`**: re-emit a one-line snapshot on directory change (notify
  crate) — the compact format makes this plain NDJSON.
- **`caldir calendars --json`**: slugs, names, colors — the normalized home
  for calendar metadata; just another view.
- **`status` / `invites` / `doctor` views**: same pattern, one view each,
  whenever wanted.
- **NO_COLOR / TTY detection for text output**: ANSI codes currently survive
  piping; fix independently (owo-colors `supports-color`). JSON sidesteps it.
- **Structured serde on `Event` itself**: unblocked by commit 1 if core ever
  wants it; not needed for this feature.
