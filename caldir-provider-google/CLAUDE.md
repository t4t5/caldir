## Design Decisions

### Provider spec

Providers let the user read and write calendar data on a remote host (e.g. Google Calendar).

Providers should be as minimal as possible and implement the following actions:
- `connect` — Multi-step connection flow (returns `NeedsInput` or `Done`)
- `list_calendars`
- `list_events`
- `create_event`
- `update_event`
- `delete_event`

The `connect` command drives a state machine: the CLI calls it in a loop, each time sending back data gathered from the previous step. This decouples auth UI from the provider, allowing different frontends (CLI, GUI) to control the user experience while supporting different auth mechanisms (OAuth, app passwords, CalDAV credentials).

There should be *no* stateful side effects from the logic in provider libraries. They should only take JSON data IN and return JSON data out.

### OAuth Authentication Modes

The Google provider supports two authentication modes:

**Hosted auth (default):** When no `app_config.toml` exists, `connect` returns `NeedsInput` with `HostedOAuth` step pointing to `caldir.org/auth/google/start`. The caldir.org relay handles the OAuth flow (holding client_id/secret server-side), exchanges the authorization code for tokens, and redirects them to the local CLI. Token refresh goes through `caldir.org/auth/google/refresh`. Sessions are saved with `auth_mode = "hosted"`.

**Self-hosted auth (`--hosted=false`):** When the user runs `caldir connect google --hosted=false` and no `app_config.toml` exists, `connect` returns `NeedsInput` with `NeedsSetup` step with instructions for creating Google Cloud OAuth credentials. After setup, the next `connect` call returns `OAuthRedirect` with a direct Google authorization URL. The CLI exchanges the code for tokens locally. Sessions are saved with `auth_mode = "local"`.

Both modes store tokens locally in `~/.config/caldir/providers/google/session/`. The `auth_mode` field in the session file determines how tokens are refreshed — hosted sessions refresh via caldir.org, local sessions refresh directly with Google using the user's client_id/secret.
