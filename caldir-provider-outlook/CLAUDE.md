# caldir-provider-outlook

Outlook / Microsoft 365 calendar provider for caldir-cli using the Microsoft Graph API.

## Design Decisions

### Provider spec

Providers let the user read and write calendar data on a remote host. They implement:
- `connect` — multi-step state machine (returns `NeedsInput` or `Done`)
- `list_calendars`, `list_events`, `create_event`, `update_event`, `delete_event`

Provider libraries take JSON in and return JSON out. Any persistent state (tokens, future sync tokens) lives under `~/.config/caldir/providers/outlook/`, not in caldir-core.

### OAuth Authentication Modes

Like the Google provider, Outlook supports two modes:

**Hosted auth (default):** OAuth flows through `caldir.org/auth/outlook/start` so the user doesn't need to register an Azure AD app. Token refresh goes through `caldir.org/auth/outlook/refresh`. Sessions saved with `auth_mode = "hosted"`.

**Self-hosted auth (`--hosted=false`):** User registers their own Azure AD application and provides client_id/client_secret in `app_config.toml`. Token refresh hits `login.microsoftonline.com` directly. Sessions saved with `auth_mode = "local"`.

Tokens live in `~/.config/caldir/providers/outlook/session/{account}.toml`.

### Why `/events` and not `/calendarView`

`list_events` uses `GET /me/calendars/{id}/events`, not `/calendarView`. Microsoft Graph's `calendarView` *expands* every recurring occurrence into its own event — and mints a unique synthetic `iCalUId` for each — turning one weekly meeting into ~50 indistinguishable rows that caldir would store as 50 separate `.ics` files. `/events` instead returns:

- `singleInstance` — normal one-off events
- `seriesMaster` — recurring event masters carrying their `recurrence` pattern (mapped to RRULE)
- `exception` — modified instances of a series (mapped to a `RECURRENCE-ID` override)

This matches caldir's data model (one file per logical event, RRULE for recurrence) and is the same pattern cal.com uses for its calendar-mirroring path.

### No date-range filter

We intentionally omit a date filter on `/events`. OData's `start/dateTime` filter only sees a series master's *first* occurrence, so a long-running meeting started in 2020 would be excluded from a 2026 window even though it has occurrences inside. Returning all events is correct; caldir-core enforces the ±365-day sync window when applying diffs.

### Recurring event identity (`originalStart`)

For `exception` instances, Graph returns `originalStart` as `Edm.DateTimeOffset` — a UTC ISO-8601 *string*, not a `dateTimeTimeZone` object. The string identifies which occurrence is being overridden and becomes the event's `RECURRENCE-ID`.

### Timezone normalization

Graph returns timezones as Windows names (e.g. `"GMT Standard Time"`). `from_outlook::normalize_timezone()` maps the common ones to IANA (`Europe/London`) so that ICS files use portable identifiers. Unknown names pass through unchanged.

### Possible future: delta sync

When polling cost becomes a concern, switch to `GET /me/calendars/{id}/events/delta` and store the resulting `@odata.deltaLink` per-calendar in `SessionData`. The protocol exposed to caldir-core does not need to change — provider-internal state stays inside the provider.
