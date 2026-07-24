# Generic CalDAV provider

The generic CalDAV provider implements RFC 4791 over HTTP basic authentication.
It works with standards-compatible services such as Fastmail, Nextcloud,
Radicale, and mailcow.

## Sync behavior

Event `UID` values are used directly for CalDAV resources, without a separate
provider event ID.

Calendar writability is detected from `DAV:current-user-privilege-set` via
PROPFIND. Calendars without write or bind privileges are exposed as read-only.

The provider's core CalDAV operations are also used by the iCloud provider.
