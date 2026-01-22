## Design Decisions

### Provider spec

Providers let the user read and write calendar data on a remote host (e.g. Google Calendar).

Providers should be as minimal as possible and implement the following actions:
- `auth_init` — Returns auth requirements (OAuth URL + state, or credential fields)
- `auth_submit` — Completes auth with gathered credentials (OAuth code or form data)
- `list_calendars`
- `list_events`
- `create_event`
- `update_event`
- `delete_event`

The two-phase auth protocol (`auth_init` + `auth_submit`) decouples auth UI from the provider, allowing different frontends (CLI, GUI) to control the user experience while supporting different auth mechanisms (OAuth, app passwords, CalDAV credentials).

There should be *no* stateful side effects from the logic in provider libraries. They should only take JSON data IN and return JSON data out.

### User-Provided OAuth Credentials

We don't embed Google Cloud credentials in the app. Users create their own Google Cloud project and provide their own client ID and secret.

This is more friction (~10 minutes of setup), but it means:
- No dependency on any third party
- No "unverified app" warnings (it's your own app)
- No single point of failure if a developer's project gets banned

