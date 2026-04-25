# caldir-provider-caldav

Generic CalDAV provider for caldir-cli. Speaks plain CalDAV (RFC 4791) with HTTP basic auth, so it works with Fastmail, Nextcloud, Radicale, mailcow, and other self-hosted/standards-compliant servers.

## Shared CalDAV ops

The pure CalDAV operations in `src/ops.rs` (`list_calendars_raw`, `fetch_events`, `create_event`, `update_event`, `delete_event`, `discover_endpoints`) are provider-agnostic and reused by `caldir-provider-icloud`. They take credentials and URLs as parameters and return `caldir-core` types — no provider-specific state.

Each provider then wraps these ops in its own `commands/list_calendars.rs` etc., adding provider-specific concerns (e.g. iCloud's `#RRGGBBAA → #RRGGBB` color normalization, Apple-ID-based session keys).

Custom `DavRequest` impls — `GetCalendarResourcesInRange`, `FindEventByUid`, `GetCurrentUserPrivilegeSet` — live in `src/caldav.rs` for things `libdav` doesn't expose directly.

## Read-only detection

For each calendar, `list_calendars_raw` issues a `PROPFIND` (Depth: 0) for `DAV:current-user-privilege-set` (RFC 3744). A calendar is reported as writable if the response contains any of the privileges `all`, `write`, or `bind` — `bind` is the privilege actually required to create new resources in a collection. Otherwise it's flagged read-only and stored as `read_only = true` in `.caldir/config.toml`.

If the server doesn't return the property, `read_only` is left as `None` and the calendar is treated as writable by default — matches `connect.rs`'s existing fallback. This means the change is graceful for servers that don't implement RFC 3744's ACL property.

Most modern CalDAV servers (Fastmail, Nextcloud, Radicale, iCloud) expose this property. The check is also useful for things like holiday calendars that some servers expose as `DAV:read-only` collections.
