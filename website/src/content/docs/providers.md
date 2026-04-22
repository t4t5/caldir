---
title: Providers
description: Provider plugin architecture for Google, iCloud, Outlook, CalDAV, and Webcal
order: 4
---

# Providers

caldir uses a plugin architecture for calendar providers. Each provider is a separate binary that communicates with caldir via JSON over stdin/stdout, similar to git's remote helpers.

## Available providers

| Provider | Binary | Auth method |
|---|---|---|
| Google Calendar | `caldir-provider-google` | OAuth (hosted/self-hosted) |
| Outlook | `caldir-provider-outlook` | OAuth (hosted/self-hosted) |
| iCloud | `caldir-provider-icloud` | App-specific password |
| Generic CalDAV | `caldir-provider-caldav` | Username + password |
| Webcal (ICS feeds) | `caldir-provider-webcal` | None (public URLs) |

## Google Calendar

### Hosted auth (recommended)

```bash
caldir connect google
```

OAuth is handled via caldir.org — no setup needed. Your tokens pass through caldir.org during authentication and refresh but are **never stored or logged** on the server. See the [privacy policy](/privacy) for details.

### Self-hosted auth (more complex)

If you prefer to use your own Google Cloud credentials:

```bash
caldir connect google --hosted=false
```

This will prompt you to create OAuth credentials in Google Cloud Console. In this mode, caldir.org is not involved at all.

## iCloud

```bash
caldir connect icloud
```

iCloud uses CalDAV with app-specific passwords. You'll be prompted to enter your Apple ID and an [app-specific password](https://support.apple.com/en-us/102654) (not your main Apple ID password).

## Outlook

Install the Outlook provider first (it's not included in the default `install.sh` bundle):

```bash
cargo install caldir-provider-outlook
```

### Hosted auth (recommended)

```bash
caldir connect outlook
```

OAuth is handled via caldir.org, similar to Google — no setup needed.

### Self-hosted auth (more complex)

If you prefer to use your own Azure AD app credentials:

```bash
caldir connect outlook --hosted=false
```

This will prompt you to register an app in the Azure portal and provide a client ID and secret.


## Other CalDAV server

```bash
caldir connect caldav
```

For any CalDAV-compatible server (Nextcloud, Radicale, Baikal, etc.). You'll be prompted for a server URL, username, and password. The provider automatically discovers CalDAV endpoints from the server.

## Webcal (ICS feeds)

```bash
caldir connect webcal
```

Subscribe to any public ICS calendar feed (`webcal://` or `https://` URLs). You'll be prompted for the feed URL — the provider validates it by fetching the feed and checking for valid ICS data.

Webcal subscriptions are **read-only**: you can pull events, but `caldir push` won't modify the remote feed. No credentials are stored — the feed URL itself is the only configuration.

Common uses: public holiday calendars, sports schedules, shared team calendars published as ICS feeds.

## Plugin architecture

Providers are discovered by looking for executables named `caldir-provider-{name}` in your PATH. This enables:

- **Permissionless ecosystem** — anyone can create a new provider
- **Language-agnostic** — providers can be written in any language
- **Independent versioning** — providers update separately from core
- **Smaller core binary** — provider-specific dependencies stay in provider crates

### Provider protocol

Providers communicate via JSON over stdin/stdout. The protocol is simple: the CLI sends `{command, params}` and the provider responds with a JSON result.

Commands:
- `connect` — authenticate with the provider (multi-step state machine)
- `list_calendars` — list all calendars for an account
- `list_events` — list events in a calendar within a time range
- `create_event` — create a new event
- `update_event` — update an existing event
- `delete_event` — delete an event

Each provider manages its own state (credentials, tokens) in `~/.config/caldir/providers/{name}/`.
