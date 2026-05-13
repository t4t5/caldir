---
name: migrate-provider
description: Migrate a caldir provider crate from the old `remote::protocol` dispatch surface to the new `provider::Handler` + `rpc` surface. Use when a provider is still commented out of the workspace `members` list, still imports from `caldir_core::remote::protocol::*`, or still hand-rolls a stdin/stdout dispatch loop in `main.rs`. Trigger: "/migrate-provider", "refactor caldir-provider-X to use new caldir-core", "bring X back into the workspace".
user_invocable: true
---

# /migrate-provider — Port a provider to the new caldir-core surface

`caldir-provider-webcal` is the **reference implementation** of the new pattern. When migrating, open it side-by-side and copy the shape — not the contents — into the target crate. `docs/providers.md` is the written contract.

If you're unsure whether a file should change: diff its shape against the equivalent file in webcal.

## Source vs. target at a glance

| | Old (pre-migration) | New (target) |
|---|---|---|
| Imports | `caldir_core::remote::protocol::*` | `caldir_core::rpc::*` |
| Event type | `caldir_core::event::Event` (private) | `caldir_core::Event` |
| ICS helpers | `caldir_core::ics::{generate_ics, parse_event}` | `event.to_ics_string()`, `Event::from_ics_str(...)` |
| `main.rs` | Hand-rolled stdin/stdout loop with generic `dispatch::<C, _, _>(...)` | `impl provider::Handler` + `provider::run_provider(...)` |
| Handler signatures | `fn handle(ctx: ProviderRequestContext, cmd: X) -> Result<Y>` | `fn handle(cmd: X) -> Result<Y>` |
| Remote-config wrapper | `TryFrom<&serde_json::Map<String, serde_json::Value>>`, `From<X> for RemoteConfig` | `TryFrom<&RemoteConfigParams>`, `into_remote_config_params(self) -> RemoteConfigParams` |
| Flattened-params field | `cmd.remote_config` | `cmd.remote` |
| Session storage dir | Passed via `ctx.provider_dir` | Resolved via `CALDIR_PROVIDER_STORAGE_DIR` env var, fall back to `~/.config/caldir/providers/{name}/` |
| `Event.uid` | `String` | `EventUid` newtype — use `.as_str()` for `&str` slots |
| Workspace `Cargo.toml` | Crate commented out of `members` | Crate listed in `members` |
| Crate `Cargo.toml` | No `async-trait`; possibly `toml = "0.9"` | Add `async-trait = "0.1"`; bump `toml = "1"` to match workspace |

## Steps

### 1. Read the reference first

Open these in order — don't migrate without them:
- `caldir-provider-webcal/src/main.rs` — the exact `Handler` impl shape
- `caldir-provider-webcal/src/remote_config.rs` — the `TryFrom<&RemoteConfigParams>` + `into_remote_config_params` pattern
- `caldir-provider-webcal/src/commands/connect.rs` — the credentials-step shape (uses `caldir_core::rpc::{ConnectStepKind, CredentialField, CredentialsData, FieldType}`)
- `caldir-provider-webcal/src/commands/list_events.rs` — the `cmd.remote` field usage
- `caldir-core/src/provider/handler.rs` — `Handler` trait, default impls, `run_provider`
- `caldir-core/src/rpc/` — the request/response struct shapes (every one flattens `remote: RemoteConfigParams`)
- `docs/providers.md` — written contract

### 2. Rewrite `main.rs`

Replace the entire dispatch loop with a `Handler` impl. Only implement the methods the provider actually supports — every method except `connect` has a default "unsupported" impl. Pattern:

```rust
mod commands;
mod constants;
mod remote_config;
mod session; // only if the provider stores credentials

use async_trait::async_trait;
use caldir_core::rpc::{Connect, ConnectResponse, /* …only what's used… */};
use caldir_core::{CalendarConfig, Event, provider};

struct FooProvider;

#[async_trait]
impl provider::Handler for FooProvider {
    async fn connect(&self, cmd: Connect) -> provider::Result<ConnectResponse> {
        Ok(commands::connect::handle(cmd).await?)
    }
    // …one method per supported RPC, each one line
}

#[tokio::main]
async fn main() {
    provider::run_provider(FooProvider).await
}
```

The `Ok(... ?)` wrapper is what bridges `anyhow::Result<T>` from the handler to `provider::Result<T>` (= `Result<T, Box<dyn StdError + Send + Sync>>`). `run_provider` walks `.source()` chains so `anyhow::Context` strings show up in error responses.

### 3. Rewrite `remote_config.rs`

Mirror webcal exactly. Drop the old `RemoteConfig` / `serde_json::Map` imports:

```rust
use caldir_core::RemoteConfigParams;

impl FooRemoteConfig {
    pub fn into_remote_config_params(self) -> RemoteConfigParams {
        let mut params = RemoteConfigParams::new();
        params.insert("foo_account".to_string(), toml::Value::String(self.foo_account));
        // … one insert per typed field
        params
    }
}

impl TryFrom<&RemoteConfigParams> for FooRemoteConfig {
    type Error = anyhow::Error;
    fn try_from(params: &RemoteConfigParams) -> anyhow::Result<Self> {
        let foo_account = params
            .get("foo_account")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required field: foo_account"))?
            .to_string();
        Ok(Self { foo_account })
    }
}
```

Any caller that used the old `From<FooRemoteConfig> for RemoteConfig` becomes `FooRemoteConfig::new(...).into_remote_config_params()`.

### 4. Update every `commands/*.rs`

Per file:
- Change `use caldir_core::remote::protocol::{...}` → `use caldir_core::rpc::{...}`.
- Change `use caldir_core::event::Event` → `use caldir_core::Event`.
- Change signature `fn handle(context: ProviderRequestContext, cmd: X) -> Result<Y>` → `fn handle(cmd: X) -> Result<Y>`.
- Replace `&cmd.remote_config` → `&cmd.remote` for `ListEvents`/`CreateEvent`/`UpdateEvent`/`DeleteEvent`.
- For `connect.rs`: `CredentialField`, `CredentialsData`, `FieldType`, `ConnectStepKind`, `ConnectResponse` all come from `caldir_core::rpc::{...}` now. `cmd.data` is still `serde_json::Map<String, serde_json::Value>` — no change.
- For `list_calendars.rs`: when building each `CalendarConfig`, use `CaldavRemoteConfig::new(...).into_remote_config_params()` and wrap with `RemoteConfig::new(ProviderSlug::from(PROVIDER_NAME), params)`. `CalendarConfig::new(name, color, read_only, Some(remote_config))` takes individual fields, not a struct literal.
- Anywhere you pass `&event.uid` to a function expecting `&str`: change to `event.uid.as_str()` (the `uid` field is now an `EventUid` newtype).

### 5. Rewrite `session.rs` if the provider has one

Drop every `&ProviderRequestContext` parameter. Add a private helper:

```rust
fn storage_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("CALDIR_PROVIDER_STORAGE_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home)
        .join(".config/caldir/providers")
        .join(PROVIDER_NAME))
}
```

`Session::save(&self)`, `Session::load(account_identifier)`, `Session::path()` etc. all call `storage_dir()?` internally. **Preserve** existing slug derivation logic byte-for-byte — users have session files on disk under those filenames. **Preserve** the `0o600` chmod on Unix.

If the provider has no on-disk state (webcal-style), skip this whole file.

### 6. Update `Cargo.toml`

- Add `async-trait = "0.1"` if missing.
- Bump `toml` to `"1"` if it's older — workspace + caldir-core use toml 1.x, mismatches surface as `expected Value, found Value` errors in remote_config.
- Don't touch other deps unless step 9 forces it.

### 7. Re-add the crate to the workspace

In root `Cargo.toml`, add the crate to `members`. The full list is preserved in a commented-out line above the active one — uncomment the relevant entry.

### 8. `cargo check -p <provider>` and iterate

Most remaining errors fall into the table at the top of this skill. Two non-obvious ones:

- **`expected Value, found Value`** in remote_config.rs → toml version mismatch (step 6).
- **`private module 'event'`** → swap `caldir_core::event::Event` for `caldir_core::Event` at the crate root.

### 9. Handle shared library drift (if any)

If the provider has a `lib.rs` exposing modules consumed by sibling crates (e.g. `caldir-provider-caldav` exposes `ops` and `caldav` to `caldir-provider-icloud`), those library modules may have accumulated drift while the crate was out of the workspace. Common cases:

- **`caldir_core::ics::{generate_ics, parse_event}` is gone.** Replace with `event.to_ics_string()` (now `pub` in caldir-core) and a small local helper over `Event::from_ics_str(...).into_iter().find_map(Result::ok)`.
- **External-crate API drift** (e.g. libdav 0.10.5 changed `DavRequest::prepare_request` to take `Uri` and return `Request<String>`). Update against the actual crate source under `~/.cargo/registry/src/.../<crate>-<version>/src/` — don't guess from docs.

**Do not** touch shared-library files unless they actually fail to compile. They may be consumed by sibling providers that haven't been migrated yet; gratuitous changes risk breaking them silently.

### 10. Mirror webcal's testability

Extract pure logic out of each command handler so it can be unit-tested without IO. Webcal does this in two places:

- `commands/connect.rs::build_calendar_config(body, url) -> Result<CalendarConfig>` — pure, fully tested
- `commands/list_events.rs::filter_events(body, from, to) -> Vec<Event>` — pure helper used by tests to bypass HTTP

Apply the same shape in the target crate. The natural seams are usually:

| Seam | Why pure |
|---|---|
| `remote_config::try_from` / `into_remote_config_params` | Pure data conversion |
| `commands/list_calendars::raw_to_config(account_id, raw)` | Map a wire-format calendar to `CalendarConfig` |
| Date/URL formatters in any helper module | No IO |
| Permission/privilege parsing in `ops` | No IO |
| ICS parsing wrappers | No IO |

Add `#[cfg(test)] mod tests` co-located in each file. **Don't** try to test save/load of session files — env-var manipulation is flaky under parallel test runs and the value you get from those tests is low.

### 11. Verify end-to-end

```sh
# Workspace builds (just pre-existing warnings remain)
cargo check

# Provider-specific build + tests
cargo check -p <provider>
cargo test -p <provider>

# Smoke RPC: connect-init should return NeedsInput (or Done for single-calendar)
echo '{"command":"connect","params":{"options":{},"data":{}}}' | cargo run -q -p <provider>

# Storage-dir override works
CALDIR_PROVIDER_STORAGE_DIR=/tmp/test cargo run -q -p <provider> < some-input.json
```

For session-bearing providers, an end-to-end check with a real account (`caldir connect <provider>` after `cargo install --path <provider>`) is the only way to validate the credential flow.

## Common pitfalls

- **Don't widen the migration scope.** The user asked to migrate one provider, not to clean up sibling providers or change the protocol. Library files like `ops.rs` that are consumed by other crates should only be touched where they literally fail to compile.
- **Don't invent ICS helpers.** `Event::to_ics_string()` is the only path; if it's still `pub(crate)` in caldir-core, make it `pub` (it already is post-migration of caldav).
- **Don't preserve `From<X> for RemoteConfig`.** The new `RemoteConfig` is constructed differently — callers should use `RemoteConfig::new(ProviderSlug::from(PROVIDER_NAME), params)`.
- **Don't try to keep `ProviderRequestContext`** as an "optional" parameter "for compatibility." It's gone; threading it through is dead weight.
- **Don't mock IO in tests.** Webcal tests parsing and filtering against static ICS strings — same pattern here. If a function only makes sense with a live server, it doesn't get a unit test.

## When this skill does NOT apply

- The provider already imports from `caldir_core::rpc::*` and uses `provider::Handler` → already migrated.
- The user wants to add a brand-new provider from scratch → use `docs/providers.md` directly, not this skill.
- The user wants to change the protocol itself (e.g. add a new RPC method) → that's a caldir-core change, not a provider migration.
