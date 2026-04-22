---
title: Providers
description: Provider plugin architecture for Google, iCloud, Outlook, CalDAV, and Webcal
order: 4
---

# Providers

caldir uses a plugin architecture for calendar providers.

## Available providers

| Provider | Binary | Auth method |
|---|---|---|
| Google Calendar | `caldir-provider-google` | OAuth (hosted/self-hosted) |
| Outlook | `caldir-provider-outlook` | OAuth (hosted/self-hosted) |
| iCloud | `caldir-provider-icloud` | App-specific password |
| Generic CalDAV | `caldir-provider-caldav` | Username + password |
| Webcal (ICS feeds) | `caldir-provider-webcal` | None (public URLs) |

## Google Calendar

### Hosted auth (easy)

```bash
caldir connect google
```

In this mode, OAuth is handled by the caldir.org server for simplicity. Your tokens are never stored or logged on the server (see [source code](https://github.com/t4t5/caldir/blob/main/website/functions/auth/google/start.ts) and [privacy policy](/privacy) for details).

### Self-hosted auth (harder)

If you prefer to use your own Google Cloud credentials instead of going through caldir.org, you can run:

```bash
caldir connect google --hosted=false
```

This will prompt you to create OAuth credentials in Google Cloud Console and set up the right
permissions.

## iCloud

```bash
caldir connect icloud
```

This will prompt you to enter your Apple ID and an [app-specific password](https://support.apple.com/en-us/102654) (*not* your main Apple ID password).

## Outlook

Install the Outlook provider first (it's not included in the default `install.sh` bundle):

```bash
cargo install caldir-provider-outlook
```

### Hosted auth (easy)

```bash
caldir connect outlook
```

OAuth is handled via caldir.org, similar to the Google provider, with no setup needed.

### Self-hosted auth (harder)

If you prefer to use your own Azure AD app credentials:

```bash
caldir connect outlook --hosted=false
```

This will prompt you to register an app in the Azure portal and provide a client ID and secret.


## Other CalDAV server

Use this to connect to any other CalDAV-compatible server (Nextcloud, Radicale, Baikal...)

```bash
caldir connect caldav
```

You'll be prompted for a server URL, username, and password.

## Webcal (public ICS feeds)

Subscribe to any public ICS calendar feed (`webcal://` or `https://` URLs).

```bash
caldir connect webcal
```

Webcal subscriptions are **read-only**: you can pull events, but `caldir push` won't modify the remote feed. No credentials are stored — the feed URL itself is the only configuration.

Common uses: public holiday calendars, sports schedules, shared team calendars published as ICS feeds.

Example feed: [Public US holidays](https://calendar.google.com/calendar/ical/en.usa%23holiday%40group.v.calendar.google.com/public/basic.ics)

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
