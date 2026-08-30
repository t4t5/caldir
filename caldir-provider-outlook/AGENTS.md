# caldir-provider-outlook

Outlook / Microsoft 365 provider via the Microsoft Graph API. Same provider contract as the others — JSON in, JSON out, all state under `~/.config/caldir/providers/outlook/`.

## Auth modes

Same two-mode setup as Google: **hosted** (OAuth flows through `caldir.org`, no Azure AD app needed) or **self-hosted** (`--hosted=false`, user registers their own app and ships client_id/secret in `app_config.toml`). The mode is recorded on the session so refresh knows which endpoint to hit.

## Calendar view discovery vs. event data

`list_events` uses `calendarView` only as a window-bounded discovery index, selecting event IDs and types. Its expanded occurrences are never converted into caldir events: occurrence and exception rows point back to their series master, which is fetched in full from `/events/{id}`. This preserves caldir's natural shape of one recurring master with its `recurrence` pattern, plus exception overrides fetched from `/instances`.

Do not replace discovery with a date filter on `/events`. OData filters only see a series master's *first* occurrence, so a long-running meeting started in 2020 would be excluded from a 2026 window.

## Recurring identity

Exception instances arrive with `originalStart` as a UTC ISO-8601 string (`Edm.DateTimeOffset`), not a `dateTimeTimeZone` object. That string identifies which occurrence is being overridden and becomes the event's `RECURRENCE-ID`.

## Timezones

Graph speaks Microsoft Windows zone names (`"GMT Standard Time"`). Caldir-core's `tz_normalize` module handles both directions — inbound `tz_normalize::normalize` maps to IANA, outbound `tz_normalize::from_iana` maps back. The same module is also reached by ICS-bytes paths (Outlook publish-calendar feeds, Windows-authored `.ics` files), so any TZID parsing benefits regardless of how the event entered.
