# caldir ICS Format Spec

This documents the iCalendar fields that caldir uses and the decisions behind them.

Reference: [RFC 5545](https://datatracker.ietf.org/doc/html/rfc5545)

---

## VCALENDAR (Container)

### `VERSION`
**Value:** `2.0`
**Why:** Required by spec. Always 2.0 for iCalendar.

### `PRODID`
**Value:** `CALDIR`
**Why:** Required by spec. Identifies the product that created the file.

**Omitted:** `CALSCALE` (GREGORIAN is the default), `SOURCE` (remote info lives in `.caldir/config.toml` instead).

---

## VEVENT (Event)

### Required Fields

#### `UID`
**What:** Unique identifier for the event.
**How caldir uses it:** We use the RFC 5545 UID (Google's `iCalUID`, CalDAV's `UID`). For recurring events, the master and all instance overrides share the same UID, linked via `RECURRENCE-ID`.
**Provider-specific IDs:** Provider-specific event IDs (e.g., Google's `id`) are stored in custom properties like `X-GOOGLE-EVENT-ID` for API calls, but the ICS UID is always the RFC 5545 UID.

#### `DTSTAMP`
**What:** Timestamp of when the ICS was created/modified.
**How caldir uses it:** We use the event's `updated` timestamp from the provider. If unavailable (e.g., birthday events), we fall back to current time.
**Why:** Required by RFC 5545. Some calendar apps validate this.
**Sync note:** Since the fallback uses current time, DTSTAMP can vary between ICS generations for events without an `updated` timestamp. Our sync comparison logic filters out DTSTAMP lines to avoid false positives—only actual content changes trigger updates.

#### `DTSTART`
**What:** When the event starts.
**How caldir uses it:**
- UTC datetime: `DTSTART:20250320T150000Z`
- Floating datetime (local time): `DTSTART:20250320T150000`
- Zoned datetime: `DTSTART;TZID=America/New_York:20250320T150000`
- All-day events: `DTSTART;VALUE=DATE:20250320`

**Timezone handling:** We preserve the original timezone format from ICS files for round-tripping. Events from Google come as UTC. Locally-created events use floating time (no timezone suffix). Events with TZID are preserved as-is.

**Note:** We don't generate VTIMEZONE components—we rely on the TZID parameter referencing standard timezone names (IANA timezone database). Most modern calendar apps resolve these without needing embedded VTIMEZONE definitions.

#### `DTEND`
**What:** When the event ends.
**How caldir uses it:** Same format as DTSTART. We always use DTEND, never DURATION.
**Tradeoff:** DURATION would be more compact for some events, but DTEND is more explicit and widely supported.

---

### Core Fields

#### `SUMMARY`
**What:** Event title.
**How caldir uses it:** Direct passthrough from provider. Also used to generate the filename slug.

#### `DESCRIPTION`
**What:** Event description/notes.
**How caldir uses it:** Optional. Direct passthrough if present.

#### `LOCATION`
**What:** Where the event takes place.
**How caldir uses it:** Optional. Direct passthrough as plain text.
**Tradeoff:** Some providers (Apple) use `X-APPLE-STRUCTURED-LOCATION` for rich location data with coordinates. We don't preserve this yet—just the plain text location.

#### `STATUS`
**What:** Event status.
**Values:** `CONFIRMED`, `TENTATIVE`, `CANCELLED`
**How caldir uses it:** Maps directly from provider status. Only emitted for TENTATIVE or CANCELLED—CONFIRMED is the implied default and is omitted to reduce file size.

#### `TRANSP`
**What:** Transparency—whether the event blocks time on your calendar.
**Values:** `OPAQUE` (busy) or `TRANSPARENT` (free)
**How caldir uses it:** Maps from Google's transparency field. Only emitted when TRANSPARENT—OPAQUE is the RFC 5545 default and is omitted.
**Why it matters:** Affects free/busy scheduling. Birthday events are typically TRANSPARENT.

---

### Recurrence Fields

#### `RRULE`
**What:** Recurrence rule defining the pattern.
**Example:** `RRULE:FREQ=WEEKLY;BYDAY=MO,WE,FR`
**How caldir uses it:** Passthrough from provider. Only present on master recurring events.
**Tradeoff:** We fetch with `single_events=false` from Google to get the actual RRULE instead of expanded instances. This means we get fewer events (just masters + modified instances) but preserve the recurrence pattern.

#### `EXDATE`
**What:** Exception dates—occurrences that were deleted.
**Example:** `EXDATE:20250320T150000Z`
**How caldir uses it:** Passthrough from provider's recurrence array.

#### `RECURRENCE-ID`
**What:** Identifies which occurrence of a recurring event this is (for instance overrides).
**Example:** `RECURRENCE-ID:20250320T150000Z`
**How caldir uses it:** Set when an event has `recurrence_id` (meaning it's a modified instance of a recurring event).
**Why:** Lets calendar apps know this file modifies a specific occurrence of a recurring series.

---

### Sync Infrastructure

#### `LAST-MODIFIED`
**What:** When the event was last changed.
**How caldir uses it:** From provider's `updated` timestamp.
**Why it matters:** Essential for future two-way sync—determines which version wins in conflicts.

#### `SEQUENCE`
**What:** Revision number. Increments each time the event is modified.
**How caldir uses it:** From provider's sequence number.
**Why it matters:** Another conflict resolution signal. Higher sequence = newer version.

---

### People

#### `ORGANIZER`
**What:** Who created/owns the meeting.
**Format:** `ORGANIZER;CN=John Doe:mailto:john@example.com`
**How caldir uses it:** Includes CN (display name) if available.
**Important:** ORGANIZER does NOT have PARTSTAT—they're the organizer, not an attendee.

#### `ATTENDEE`
**What:** Meeting participants.
**Format:** `ATTENDEE;CN=Jane Doe;PARTSTAT=ACCEPTED:mailto:jane@example.com`
**How caldir uses it:** Includes:
- `CN` - Display name
- `PARTSTAT` - Response status (ACCEPTED, DECLINED, TENTATIVE, NEEDS-ACTION)

**Tradeoff:** We don't include ROLE, RSVP, CUTYPE, or other attendee parameters. These are rarely used in practice and add complexity. Other libraries like ics-py and khal include these, but they don't affect behavior in most calendar apps.

---

### Alarms

#### `VALARM` (component)
**What:** Reminder/notification for the event.
**How caldir uses it:**
```
BEGIN:VALARM
ACTION:DISPLAY
DESCRIPTION:Reminder
TRIGGER:-PT10M
END:VALARM
```

**Fields we use:**
- `ACTION:DISPLAY` - Always display type (not email/audio)
- `TRIGGER` - Minutes before event (e.g., `-PT10M` = 10 min before)
- `DESCRIPTION` - Generic "Reminder" text (required by RFC 5545 for DISPLAY alarms)

**Minimal by design:** RFC 5545 only requires ACTION, TRIGGER, and DESCRIPTION for display alarms. We omit UID and DTSTAMP (which the icalendar crate would auto-add) via post-processing since they're not required and add bloat.

**Tradeoff:** Google has both "default reminders" (calendar-level) and "override reminders" (event-level). We only sync override reminders. If an event uses default reminders, it won't have any VALARM in the ICS file.

---

### Conference/Video Calls

#### `URL`
**What:** Standard URL field.
**How caldir uses it:** Set to the video conference URL (Google Meet, etc.) if present.

#### `X-GOOGLE-EVENT-ID`
**What:** Google-specific extension storing Google's internal event ID.
**How caldir uses it:** Stored in `custom_properties` when pulled from Google. Used for API calls (updates, deletes) since Google's API requires its own event ID, not the RFC 5545 UID.

#### `X-GOOGLE-CONFERENCE`
**What:** Google-specific extension for conference links.
**How caldir uses it:** Preserved in `custom_properties` when pulled from Google, enabling round-trip sync. We don't actively generate this field—only `URL` is set from the conference URL.

---

## Deterministic Generation

ICS files must be generated deterministically for sync to work correctly. If the same event data produces different ICS output on each generation, the sync logic would see false positives (detecting "changes" when nothing actually changed).

### Sources of Non-Determinism

The icalendar crate can introduce non-determinism by auto-generating fields:

1. **Auto-generated DTSTAMP** - If no DTSTAMP is set, the crate uses `Utc::now()`
2. **Auto-generated UID** - If no UID is set, the crate generates a random UUID

### How We Handle It

**At generation time:**
- Event UID: Set from provider's event ID (deterministic)
- Event DTSTAMP: Set from provider's `updated` timestamp when available

**Post-processing:**
- Strip CALSCALE:GREGORIAN (it's the default, no need to emit)
- Strip UID and DTSTAMP from VALARM components (not required by RFC 5545)

**At comparison time:**
- Sync uses file mtime (local) vs `updated` field from API (remote)
- Event content comparison uses our custom PartialEq which excludes `updated`, `sequence`, and `custom_properties`

---

## Fields We Skip

These are valid iCalendar fields we intentionally don't use:

| Field | Why we skip it |
|-------|----------------|
| `CREATED` | Informational only, doesn't affect behavior |
| `CLASS` | PUBLIC/PRIVATE/CONFIDENTIAL—most apps ignore it |
| `PRIORITY` | 0-9 priority level—almost never used |
| `CATEGORIES` | Tags/labels—few apps support them |
| `GEO` | Lat/long—apps prefer the LOCATION string |
| `ATTACH` | File attachments—better to link than embed |
| `RESOURCES` | Room/equipment booking—very niche |
| `RDATE` | Extra recurrence dates—RRULE+EXDATE covers 99% of cases |
| `CONTACT` | Contact info—ORGANIZER is sufficient |
| `COMMENT` | Extra comments—rarely used |
| `VTIMEZONE` | Timezone definitions—we use TZID parameter with IANA names instead |

---

## Filename Convention

caldir uses semantic filenames instead of UUIDs:

**Regular events:** `{date}__{slug}.ics`
- Example: `2025-03-20T1500__team-standup.ics`
- Example (all-day): `2025-03-21__company-offsite.ics`

**Recurring masters:** `_recurring__{slug}.ics`
- Example: `_recurring__weekly-standup.ics`
- No date prefix because the master represents all occurrences

**Collision handling:** If multiple events have the same date/time and slug, a numeric suffix is added (`-2`, `-3`, etc.)

**Why:**
- Human-readable at a glance
- Sortable by date in file browsers
- `ls ~/caldir` shows you your schedule
- LLMs can reason about your calendar without parsing ICS

---

## Account Identifier Convention

Providers that have an account concept (e.g., Google, iCloud) should include a `{provider}_account` field in their remote config. This allows caldir consumers (like GUI apps) to group calendars by account for display purposes.

```toml
# Google calendar — has an account
[remote]
provider = "google"
google_account = "me@gmail.com"
google_calendar_id = "primary"

# iCloud calendar — has an account
[remote]
provider = "icloud"
icloud_account = "me@icloud.com"
icloud_calendar_url = "https://caldav.icloud.com/..."

# Plain CalDAV — no account
[remote]
provider = "caldav"
caldav_url = "https://example.com/dav/calendar"
```

The `Remote::account_identifier()` method in caldir-core extracts this by looking up `{provider}_account` in the config. Returns `None` for providers without accounts.

---

## Provider-Specific Notes

### Google Calendar
- We use `single_events=false` to get RRULE instead of expanded instances
- Google's event `id` is stored as `X-GOOGLE-EVENT-ID` for API calls; the ICS `UID` is Google's `iCalUID`
- Conference data comes from `conferenceData.entryPoints[type=video].uri`
- Reminders come from `reminders.overrides` (not default reminders)

### Apple/iCloud (CalDAV)
- Uses standard CalDAV protocol with app-specific passwords
- The ICS `UID` is used directly for CalDAV API calls (no separate provider ID needed)
- May need to preserve `X-APPLE-STRUCTURED-LOCATION` for rich location data
- `X-APPLE-TRAVEL-ADVISORY-BEHAVIOR` controls travel time calculations

### Future: Outlook
- `X-MICROSOFT-CDO-BUSYSTATUS` maps to TRANSP (FREE→TRANSPARENT, BUSY→OPAQUE)
- Most `X-MICROSOFT-CDO-*` fields are compatibility cruft and can be ignored

---

## Sync State File

### `.caldir/state/known_event_ids`

Each calendar directory contains a `.caldir/state/known_event_ids` file that tracks which events have been synced with the remote provider.

**Format:** Plaintext file, one event ID per line (sorted alphabetically for deterministic output):
```
abc123@google.com
abc123@google.com__20250317T100000Z
def456@icloud.com
```

Event IDs use the RFC 5545 identity:
- Non-recurring events: `{uid}` (e.g., `abc123@google.com`)
- Recurring event instances: `{uid}__{recurrence_id}` (e.g., `abc123@google.com__20250317T100000Z`)

The double underscore (`__`) separator distinguishes the recurrence_id from the uid.

**Why:** Enables the sync logic to distinguish between:
- **Locally-created events** (event ID not in known_event_ids) → candidates for pushing to cloud
- **Remotely-deleted events** (event ID in known_event_ids, but missing from remote) → candidates for local deletion

Without this state, a local-only event is ambiguous: was it created locally and needs to be pushed, or was it pulled from the cloud and then deleted remotely?

**Lifecycle:**
- After `pull`: Event IDs of all fetched events are added to known_event_ids
- After `push` (create): Newly created event IDs are added to known_event_ids
- After `pull` (delete): Removed event IDs are deleted from known_event_ids

---

## Relationship to vdir

[vdir](https://vdirsyncer.pimutils.org/en/stable/vdir.html) is an similar standard for storing calendars on a filesystem, used by vdirsyncer, khal, and other tools.

### What caldir shares with vdir

- Subdirectories represent calendars (collections)
- One `.ics` file per event
- Files contain a `UID` property

### Where caldir intentionally diverges

**Filenames:** vdir specifies opaque, UID-like filenames (`5a3c9b7e-1234-5678-abcd.ics`). caldir uses semantic filenames with embedded date and title (`2025-03-20T1500__team-standup.ics`).

**Filename stability:** vdir requires "when changing an item, the original filename must be used." caldir renames files when the event date or title changes, since the filename encodes that information.

### Why we diverge

caldir is designed for human and LLM readability. The semantic filenames mean:

- Files sort chronologically by default
- AI assistants can reason about your calendar from filenames alone, without parsing ICS
- Shell tools (grep, tab completion) work naturally

