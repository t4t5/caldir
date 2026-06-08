# caldir-provider-icloud

iCloud Calendar provider via CalDAV (RFC 4791). Wraps the shared CalDAV ops in `caldir-provider-caldav` with iCloud-specific concerns (Apple-ID-keyed sessions, hex color normalization, the Apple discovery flow).

## App-specific passwords

iCloud refuses the regular Apple ID password for third-party CalDAV access. Users generate a 16-character app-specific password at <https://account.apple.com> and provide that during `connect`. We surface this requirement in the connect prompt — it's the most common point of confusion.

## Endpoint discovery

Apple uses a generic `caldav.icloud.com` entry point that redirects each user to their own server (`pNN-caldav.icloud.com`). The `connect` flow walks PROPFIND from the entry point → principal URL → calendar-home URL and records all three on the session. After that, every operation hits the user's real server directly.

## Read-only detection

Shared with caldir-provider-caldav: a PROPFIND for `DAV:current-user-privilege-set` decides whether each calendar can be written. iCloud reports per-calendar privileges accurately, including for view-only shared calendars, so calendars surface in caldir with the correct read-only flag without any user configuration.
