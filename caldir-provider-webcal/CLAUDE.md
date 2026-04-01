# caldir-provider-webcal

Read-only provider for public ICS calendar feeds (webcal:// URLs).

## Design Decisions

### No Session Files

Unlike other providers, webcal does **not** store session files. Webcal feeds are public with no authentication, so there's no state to persist. Everything needed (URL, display name, color) lives in the calendar's `.caldir/config.toml`.

### Read-Only

All mutation operations (`create_event`, `update_event`, `delete_event`) return an error. You can't push changes to a public ICS feed.
