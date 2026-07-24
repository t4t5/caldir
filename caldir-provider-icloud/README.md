# iCloud Calendar provider

The iCloud provider uses CalDAV with an Apple ID and an app-specific password.

## Sync behavior

Event `UID` values are used directly for CalDAV resources, without a separate
provider event ID.

Apple extensions such as `X-APPLE-STRUCTURED-LOCATION` and
`X-APPLE-TRAVEL-ADVISORY-BEHAVIOR` are not interpreted, but round-trip
unchanged like other `X-` properties.

The provider shares its core CalDAV operations and per-calendar writability
detection with the generic CalDAV provider.
