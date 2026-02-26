# caldir-provider-icloud

iCloud Calendar provider for caldir-cli using CalDAV protocol.

## Design Decisions

### Provider spec

Providers let the user read and write calendar data on a remote host (e.g. iCloud Calendar).

Providers should be as minimal as possible and implement the following actions:
- `connect` — Multi-step connection flow (returns `NeedsInput` or `Done`)
- `list_calendars`
- `list_events`
- `create_event`
- `update_event`
- `delete_event`

The `connect` command drives a state machine: the CLI calls it in a loop, each time sending back data gathered from the previous step. This decouples auth UI from the provider, allowing different frontends (CLI, GUI) to control the user experience while supporting different auth mechanisms (OAuth, app passwords, CalDAV credentials).

There should be *no* stateful side effects from the logic in provider libraries. They should only take JSON data IN and return JSON data out.

### App-Specific Passwords

iCloud requires app-specific passwords for third-party apps. Users must:
1. Go to https://account.apple.com/sign-in
2. Sign in and navigate to Sign-In and Security → App-Specific Passwords
3. Generate a new password named "caldir"
4. Use this 16-character password when running `caldir connect icloud`

This is a security requirement from Apple - regular Apple ID passwords cannot be used for CalDAV access.

### CalDAV Protocol

iCloud uses CalDAV (RFC 4791) for calendar access. Key endpoints:
- `caldav.icloud.com` - Initial endpoint (redirects to user-specific server)
- `pXX-caldav.icloud.com` - User-specific CalDAV server
- Calendar discovery via PROPFIND on principal URL
- Events fetched via REPORT with calendar-query
- Events created/updated via PUT
- Events deleted via DELETE

### Credential Storage

Credentials are stored at `~/.config/caldir/providers/icloud/session/{apple_id_slug}.toml`:
- Apple ID (email)
- App-specific password
- Discovered principal URL
- Discovered calendar-home URL

File permissions are set to 0600 (owner-only) for security.

## Module Structure

```
src/
├── main.rs              # JSON protocol dispatcher
├── constants.rs         # PROVIDER_NAME, CALDAV_ENDPOINT
├── session.rs           # Credential storage and loading
├── remote_config.rs     # ICloudRemoteConfig type
├── caldav.rs            # CalDAV client helpers
└── commands/
    ├── mod.rs
    ├── connect.rs       # Connect flow (credential fields → validate → done)
    ├── list_calendars.rs
    ├── list_events.rs
    ├── create_event.rs
    ├── update_event.rs
    └── delete_event.rs
```

## CalDAV Request/Response Flow

### Authentication (connect)

1. PROPFIND on `caldav.icloud.com/` with `current-user-principal` property
2. Parse response to get principal URL (e.g., `/123456789/principal/`)
3. PROPFIND on principal URL with `calendar-home-set` property
4. Parse response to get calendar home URL
5. Save all discovered URLs with credentials

### List Calendars

1. PROPFIND on calendar-home URL with Depth: 1
2. Filter responses for calendar collections (resourcetype contains calendar)
3. Extract displayname and color for each calendar

### List Events

1. REPORT on calendar URL with `calendar-query`
2. Filter by time-range (from/to dates)
3. Parse VCALENDAR data from each response
4. Convert to Event structs using caldir-core ICS parser

### Create/Update Event

1. Generate ICS content from Event using caldir-core
2. PUT to `{calendar_url}/{event_uid}.ics`
3. Fetch the created/updated event to get server modifications

### Delete Event

1. DELETE `{calendar_url}/{event_uid}.ics`
2. Accept 204 (success) or 404 (already deleted)
