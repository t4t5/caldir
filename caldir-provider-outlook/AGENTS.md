# caldir-provider-outlook

Outlook / Microsoft 365 provider via the Microsoft Graph API. Same provider contract as the others — JSON in, JSON out, all state under `~/.config/caldir/providers/outlook/`.

## Auth modes

Same two-mode setup as Google: **hosted** (OAuth flows through `caldir.org`, no Azure AD app needed) or **self-hosted** (`--hosted=false`, user registers their own app and ships client_id/secret in `app_config.toml`). The mode is recorded on the session so refresh knows which endpoint to hit.

## Hybrid event discovery

`list_events` uses `calendarView` as a window-bounded discovery index for standalone events and recurring instances. A compact `/events?$select=id,type&$filter=type eq 'seriesMaster'` index separately discovers every series master, so an existing series cannot look deleted merely because none of its instances remains in the requested window. The union is fetched in full from `/events/{id}`.

Expanded `calendarView` occurrences are never converted into caldir events: occurrence and exception rows point back to their series master. Only masters represented in the requested window incur an `/instances` request for exception overrides. This preserves caldir's natural shape of one recurring master with its `recurrence` pattern plus its exceptions, without expanding every historical series.

Do not replace discovery with a date filter on `/events`. OData filters only see a series master's *first* occurrence, so a long-running meeting started in 2020 would be excluded from a 2026 window.

## Recurring identity

Exception instances arrive with `originalStart` as a UTC ISO-8601 string (`Edm.DateTimeOffset`), not a `dateTimeTimeZone` object. That string identifies which occurrence is being overridden and becomes the event's `RECURRENCE-ID`.

## Timezones

Graph speaks Microsoft Windows zone names (`"GMT Standard Time"`). Caldir-core's `tz_normalize` module handles both directions — inbound `tz_normalize::normalize` maps to IANA, outbound `tz_normalize::from_iana` maps back. The same module is also reached by ICS-bytes paths (Outlook publish-calendar feeds, Windows-authored `.ics` files), so any TZID parsing benefits regardless of how the event entered.
