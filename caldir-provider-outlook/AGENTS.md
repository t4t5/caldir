# caldir-provider-outlook

Outlook / Microsoft 365 provider via the Microsoft Graph API. Same provider contract as the others — JSON in, JSON out, all state under `~/.config/caldir/providers/outlook/`.

## Auth modes

Same two-mode setup as Google: **hosted** (OAuth flows through `caldir.org`, no Azure AD app needed) or **self-hosted** (`--hosted=false`, user registers their own app and ships client_id/secret in `app_config.toml`). The mode is recorded on the session so refresh knows which endpoint to hit.

## Why `/events`, not `/calendarView`

Graph's `calendarView` *expands* every recurring occurrence into its own event with a synthetic `iCalUId`, turning one weekly meeting into ~50 indistinguishable rows. `/events` returns the natural shape — series masters with their `recurrence` pattern, plus exception overrides — which maps cleanly to caldir's one-file-per-logical-event model.

We deliberately don't pass a date filter: OData filters only see a series master's *first* occurrence, so a long-running meeting started in 2020 would be excluded from a 2026 window. We pull everything and let core enforce the ±365-day window.

## Recurring identity

Exception instances arrive with `originalStart` as a UTC ISO-8601 string (`Edm.DateTimeOffset`), not a `dateTimeTimeZone` object. That string identifies which occurrence is being overridden and becomes the event's `RECURRENCE-ID`.

## Timezones

Graph speaks Microsoft Windows zone names (`"GMT Standard Time"`). Caldir-core's `tz_normalize` module handles both directions — inbound `tz_normalize::normalize` maps to IANA, outbound `tz_normalize::from_iana` maps back. The same module is also reached by ICS-bytes paths (Outlook publish-calendar feeds, Windows-authored `.ics` files), so any TZID parsing benefits regardless of how the event entered.
