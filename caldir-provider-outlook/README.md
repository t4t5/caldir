# Outlook / Microsoft 365 provider

The Outlook provider syncs calendars through the Microsoft Graph API.

## Sync behavior

Events are fetched from `/events`, rather than `/calendarView`, so recurring
series remain masters instead of being expanded into individual occurrences.

Microsoft Graph uses Windows timezone names. The provider normalizes them to
IANA names on input and converts them back to Windows names on output.

Recurring exception instances expose `originalStart`, which becomes the ICS
`RECURRENCE-ID`.
