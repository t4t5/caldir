---
name: provider-rpc
description: Talk to a caldir provider binary directly via its JSON protocol to debug sync issues. Use when investigating "Provider error" messages, suspected wrong/missing event data from a provider, or when you need to see exactly what a provider returns before it goes through caldir-core. Triggers: "what does the provider return for X", "why is X showing up in status", "is this a parsing bug or a Google/iCloud/Outlook bug", investigating any provider-emitted error.
user_invocable: false
---

# Talking to providers directly

Providers are standalone binaries that speak JSON over stdin/stdout. When something looks wrong in `caldir status` / `caldir pull`, you can usually narrow the bug to "wrong on the wire" vs. "wrong after parsing" in one shot by invoking the provider directly.

This bypasses caldir-cli, caldir-core, the diff engine, and the renderer — you see exactly what came back from the remote.

## Protocol shape

One JSON line in, one JSON line out. The protocol is defined in `caldir-core/src/remote/protocol.rs` — that file is the source of truth, check it if a command argument shape isn't obvious.

Request:
```json
{"command":"<command>","params":{...}}
```

Response (success):
```json
{"status":"success","data":<command-specific>}
```

Response (error):
```json
{"status":"error","error":"<message>"}
```

## Critical: params is FLAT

`remote_config` is `#[serde(flatten)]` in protocol structs. Pass its fields at the top level of `params`, not nested under `remote_config`. If you nest, you'll get `Missing required field: <name>`.

```jsonc
// CORRECT
{"command":"list_events","params":{
  "google_account":"me@gmail.com",
  "google_calendar_id":"primary",
  "from":"2026-01-01T00:00:00Z",
  "to":"2026-02-01T00:00:00Z"
}}

// WRONG — provider rejects with "Missing required field: google_account"
{"command":"list_events","params":{
  "remote_config":{"google_account":"me@gmail.com", ...},
  ...
}}
```

## Commands

All providers implement: `connect`, `list_calendars`, `list_events`, `create_event`, `update_event`, `delete_event`. See `caldir-core/src/remote/protocol.rs` for exact field names per command.

`list_events` is the workhorse for debugging — it takes `from` / `to` as RFC3339 strings.

## Finding the right config

Per-calendar config lives at `~/<calendar_dir>/<slug>/.caldir/config.toml`. The `[remote]` block has the provider name and provider-prefixed account/calendar fields. Read it with `cat` and pass the same fields (minus `provider`) into `params`.

The user's `calendar_dir` comes from `~/.config/caldir/config.toml`.

## Use the dev build, not the installed binary

After editing a provider, rebuild it and invoke the dev binary by absolute path — don't rely on `which caldir-provider-google`, that finds the installed copy in `~/.cargo/bin`.

```bash
cargo build -p caldir-provider-google
PROVIDER="$(pwd)/target/debug/caldir-provider-google"  # run from repo root
```

If you want the whole CLI to use the dev build (e.g. to confirm a fix end-to-end via `caldir status`), prepend `target/debug` to PATH:

```bash
PATH="$(pwd)/target/debug:$PATH" caldir status
```

## Pretty-printing output

`jq` is fine for shape inspection. For Google-specific filtering (e.g. "find every event referencing this UID"), Python with `json.load(sys.stdin)` and a list comprehension is faster to write than the jq equivalent:

```bash
echo '{"command":"list_events","params":{...}}' | "$PROVIDER" | python3 -c "
import json, sys
r = json.loads(sys.stdin.read())
print('status:', r.get('status'))
events = r.get('data', [])
print('total:', len(events))
for e in events:
    if 'lars' in (e.get('summary') or '').lower():
        print(json.dumps(e, indent=2))
"
```

## Going below the provider

When you suspect the provider itself is misinterpreting what the upstream API returned, hit the upstream directly. For Google:

```bash
TOKEN=$(grep "^access_token" ~/.config/caldir/providers/google/session/<account>.toml | cut -d'"' -f2)
curl -s -H "Authorization: Bearer $TOKEN" \
  "https://www.googleapis.com/calendar/v3/calendars/<id>/events?timeMin=...&timeMax=...&singleEvents=true&showDeleted=true" \
  | jq '.items[] | select(.summary | test("foo"; "i"))'
```

Useful Google flags:
- `singleEvents=true` — expand recurrences into instances; cancelled instances are omitted
- `showDeleted=true` — include cancelled instances/events (returns full data when used with `singleEvents=true`)
- Default (`singleEvents=false`, `showDeleted=false`) returns master events plus bare cancellation tombstones

Comparing the upstream response to what the provider emits is the fastest way to localize a bug to "upstream is weird" vs. "we're parsing it wrong".

## When NOT to use this

- For testing pure conversion logic (e.g. `from_google` mapping fields), prefer a unit test with a fake `google_calendar::types::Event` — see `caldir-provider-google/src/commands/list_events.rs` tests for the empty-event-via-serde pattern.
- For end-to-end behavior (status output, file writes), just run `caldir status` / `caldir pull` with the dev provider on PATH.

The direct-RPC trick is for the gap between those two: "what's actually on the wire?"
