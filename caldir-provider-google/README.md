# Google Calendar provider

## Sync behavior

Recurring events are fetched with `singleEvents=false`, preserving each series
as a master with an `RRULE` instead of expanding every occurrence.

Only explicit event reminder overrides are stored. Google calendar-level
default reminders are not copied into individual events.

## Google-specific ICS properties

### `X-GOOGLE-EVENT-ID`

Google identifies events with its own internal ID, separate from the RFC 5545
`UID`. The provider stores that ID in `X-GOOGLE-EVENT-ID` when pulling an event
and uses it for later updates and deletes.

### `X-GOOGLE-CONFERENCE`

`X-GOOGLE-CONFERENCE` stores a Google Meet URL when an event is pulled from
Google. Its value comes from the video entry point in
`conferenceData.entryPoints`. The provider rebuilds Google's conference data
from this property when the event is pushed.

An empty or whitespace-only value requests a new Meet link:

```ics
X-GOOGLE-CONFERENCE:
```

On the next push, Google creates the conference and the provider replaces the
empty property with the resulting Meet URL. The request ID combines the event
UID and sequence, making retries idempotent while allowing a later event
revision to request a different conference.

If Google rejects conference creation, the provider retries new event creation
without conference data. The empty property is then dropped during the
round-trip.
