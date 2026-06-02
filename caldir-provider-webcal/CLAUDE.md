# caldir-provider-webcal

Read-only provider for public ICS calendar feeds (`webcal://` URLs).

## Difference from other providers

No authentication, no session files. Public feeds carry no state to persist — the URL, display name, and color all live in the calendar's `.caldir/config.toml`. All mutation operations error out: there's nowhere to push to.

The calendar surfaces in caldir with `read_only = true`, so `caldir push` and `caldir sync` skip the outbound half automatically.
