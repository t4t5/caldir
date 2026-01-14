## Design Decisions

### Provider spec

Providers let the user read and write calendar data on a remote host (e.g. Google Calendar).

Providers should be as minimal as possible and implement the following actions:
- `authenticate`
- `list_calendars`
- `list_events`
- `create_event`
- `update_event`
- `delete_event`

There should be *no* stateful side effects from the logic in provider libraries. They should only take JSON data IN and return JSON data out.

### User-Provided OAuth Credentials

We don't embed Google Cloud credentials in the app. Users create their own Google Cloud project and provide their own client ID and secret.

This is more friction (~10 minutes of setup), but it means:
- No dependency on any third party
- No "unverified app" warnings (it's your own app)
- No single point of failure if a developer's project gets banned

