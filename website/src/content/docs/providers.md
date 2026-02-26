---
title: Providers
description: Provider plugin architecture, Google and iCloud setup
order: 5
---

# Providers

caldir uses a plugin architecture for calendar providers. Each provider is a separate binary that communicates with caldir via JSON over stdin/stdout, similar to git's remote helpers.

## Available providers

| Provider | Binary | Auth method |
|---|---|---|
| Google Calendar | `caldir-provider-google` | OAuth (hosted or self-hosted) |
| Apple iCloud | `caldir-provider-icloud` | App-specific password (CalDAV) |

## Google Calendar

### Hosted auth (default)

```bash
caldir connect google
```

OAuth is handled via caldir.org — no setup needed. Your tokens pass through caldir.org during authentication and refresh but are **never stored or logged** on the server. See the [privacy policy](/privacy) for details.

### Self-hosted auth

If you prefer to use your own Google Cloud credentials:

```bash
caldir connect google --hosted=false
```

This will prompt you to create OAuth credentials in Google Cloud Console. In this mode, caldir.org is not involved at all.

### Multiple accounts

You can connect multiple Google accounts by running `caldir connect google` multiple times. Each account's calendars will be synced independently.

## Apple iCloud

```bash
caldir connect icloud
```

iCloud uses CalDAV with app-specific passwords. You'll be prompted to enter your Apple ID and an [app-specific password](https://support.apple.com/en-us/102654) (not your main Apple ID password).

## Plugin architecture

Providers are discovered by looking for executables named `caldir-provider-{name}` in your PATH. This enables:

- **Permissionless ecosystem** — anyone can create a new provider (e.g., `caldir-provider-outlook`)
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
