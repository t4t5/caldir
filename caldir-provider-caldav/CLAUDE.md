# caldir-provider-caldav

Generic CalDAV provider — speaks plain RFC 4791 with HTTP basic auth. Works with Fastmail, Nextcloud, Radicale, mailcow, and any standards-compliant server.

## Shared ops

The pure CalDAV operations in `src/ops.rs` are provider-agnostic and reused by `caldir-provider-icloud`. They take credentials and URLs as parameters, return `caldir-core` types, and hold no provider-specific state. Each provider then wraps these in its own commands, layering on whatever quirks the host needs (iCloud's color normalization, Apple-ID session keys, etc.).

Custom `DavRequest` impls live in `src/caldav.rs` for things `libdav` doesn't expose.

## Read-only detection

A PROPFIND for `DAV:current-user-privilege-set` (RFC 3744) decides whether each calendar can be written. We treat any of `all`, `write`, or `bind` as writable — `bind` is the privilege actually required to create new resources in a collection.

If the server doesn't return the property, the calendar defaults to writable so older or non-RFC-3744 servers don't silently break. Most modern servers (Fastmail, Nextcloud, Radicale, iCloud) implement the property correctly, including for things like holiday calendars and view-only shares.
