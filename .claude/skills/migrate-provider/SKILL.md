---
name: migrate-provider
description: Migrate a caldir provider crate from the old `remote::protocol` dispatch surface to the new `provider::Handler` + `rpc` surface. Use when a provider is still commented out of the workspace `members` list, still imports from `caldir_core::remote::protocol::*`, or still hand-rolls a stdin/stdout dispatch loop in `main.rs`. Trigger: "/migrate-provider", "refactor caldir-provider-X to use new caldir-core", "bring X back into the workspace".
user_invocable: true
---

# /migrate-provider — Port a provider to the new caldir-core surface

The reference implementations of the new pattern, by area:

- **Overall shape** (`main.rs`, `remote_config.rs`, the `commands/*.rs` handlers): `caldir-provider-webcal`. Read-only, no session — the simplest possible incarnation of the new surface.
- **Sessions / on-disk credentials**: `caldir-provider-icloud`. The canonical `Session` (pure data) + `SessionStore` (IO with injected `ProviderStorage`) split, including the direct-path `load` shape that all future providers should mirror.
- **Shared `pub mod` library consumed by sibling providers**: `caldir-provider-caldav`. iCloud reuses its `caldav::ops` for the actual CalDAV calls.

When migrating, open the relevant one side-by-side and copy the shape — not the contents — into the target crate. `docs/providers.md` is the written contract.

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
| Session storage dir | Passed via `ctx.provider_dir` | Inject `ProviderStorage::for_provider(name)` into a provider-specific store struct; tests pass `ProviderStorage::new(tempdir)` |
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
- **If the provider stores credentials**: `caldir-provider-icloud/src/session.rs` (re-export entry), `session/types.rs` (pure `Session` + forward-deterministic slug), `session/store.rs` (`SessionStore` with direct-path `load`, chmod, tempdir-based tests) — the canonical session-storage layout
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

iCloud's session module is the canonical shape — mirror it. Three files under `src/session/`, all with co-located `#[cfg(test)] mod tests`:

**`src/session.rs`** — module entry, re-exports only:

```rust
mod store;
mod types;
pub use store::SessionStore;
pub use types::Session;
```

**`src/session/types.rs`** — pure data; no IO, no env vars, no `ProviderRequestContext`. `Session::slug` is forward-deterministic from one input (the account identifier), which is what lets the store compute paths directly instead of scanning:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub account_identifier: String,     // whatever uniquely identifies the account
    pub credential: String,             // password / token / refresh token / …
    // …any discovered URLs, expiry timestamps, etc.
}

impl Session {
    pub fn new(/* … */) -> Self { /* … */ }

    /// Filesystem slug. Forward-deterministic from the account identifier so
    /// `SessionStore::load` can compute the path directly. Preserve any
    /// existing replacement logic byte-for-byte — users have files on disk.
    pub(super) fn slug(account_identifier: &str) -> String {
        account_identifier.replace(['/', '\\', ':', '@', '.'], "_")
    }

    pub fn credentials(&self) -> (&str, &str) {
        (&self.account_identifier, &self.credential)
    }
}
```

**`src/session/store.rs`** — all IO. Holds an injected `ProviderStorage`. `load` computes the path directly (no scan); the on-disk slug is forward-deterministic from the account identifier:

```rust
use caldir_core::provider::ProviderStorage;
use super::Session;

pub struct SessionStore {
    storage: ProviderStorage,
}

impl SessionStore {
    pub fn new(storage: ProviderStorage) -> Self { Self { storage } }

    pub fn save(&self, session: &Session) -> Result<()> {
        let path = self.path_for(&session.account_identifier);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, toml::to_string_pretty(session)?)?;
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    pub fn load(&self, account_identifier: &str) -> Result<Session> {
        let path = self.path_for(account_identifier);
        if !path.exists() {
            anyhow::bail!("session for {} not found!", account_identifier);
        }
        Ok(toml::from_str(&std::fs::read_to_string(&path)?)?)
    }

    fn session_dir(&self) -> PathBuf { self.storage.root().join("session") }
    fn path_for(&self, account_identifier: &str) -> PathBuf {
        self.session_dir().join(format!("{}.toml", Session::slug(account_identifier)))
    }
}
```

Production wiring (in `commands/connect.rs` and the read-path handlers):

```rust
let store = SessionStore::new(ProviderStorage::for_provider(PROVIDER_NAME)?);
store.save(&session)?;            // or: store.load(&account_id)?
```

Tests construct the store with an explicit tempdir — no env vars, no parallel-test contention:

```rust
let tmp = tempfile::TempDir::new()?;
let store = SessionStore::new(ProviderStorage::new(tmp.path()));
```

Mirror iCloud's test set: `save_writes_toml_under_session_subdir`, `load_round_trips_by_account_identifier`, `load_errors_when_missing`, and `#[cfg(unix)] save_chmods_session_file_to_0600`. Add `tempfile = "3"` under `[dev-dependencies]`.

**Preserve** existing slug derivation logic byte-for-byte — users already have session files on disk under those filenames. **Preserve** the `0o600` chmod on Unix.

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

If the provider has a `lib.rs` exposing modules consumed by sibling crates (e.g. `caldir-provider-caldav` exposes its `caldav` module — which contains `caldav::client` and `caldav::ops` — to `caldir-provider-icloud`), those library modules may have accumulated drift while the crate was out of the workspace. Common cases:

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

Add `#[cfg(test)] mod tests` co-located in each file.

For session/token/cursor stores: follow the **pure-data + IO-struct + injected `ProviderStorage`** shape from step 5. Tests construct the store with `ProviderStorage::new(tempdir.path())` and round-trip against it. `Session` itself stays pure data, so its tests don't need a tempdir at all. The pattern generalizes — any future provider state (OAuth tokens, sync cursors, etc.) should follow the same split.

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

- **Don't widen the migration scope.** The user asked to migrate one provider, not to clean up sibling providers or change the protocol. Shared library modules consumed by other crates (e.g. `caldir-provider-caldav`'s `caldav::ops`) should only be touched where they literally fail to compile.
- **Don't invent ICS helpers.** `Event::to_ics_string()` is the only path; if it's still `pub(crate)` in caldir-core, make it `pub` (it already is post-migration of caldav).
- **Don't preserve `From<X> for RemoteConfig`.** The new `RemoteConfig` is constructed differently — callers should use `RemoteConfig::new(ProviderSlug::from(PROVIDER_NAME), params)`.
- **Don't try to keep `ProviderRequestContext`** as an "optional" parameter "for compatibility." It's gone; threading it through is dead weight.
- **Don't mock IO in tests.** Webcal tests parsing and filtering against static ICS strings — same pattern here. If a function only makes sense with a live server, it doesn't get a unit test.
- **Don't leave `mod.rs` files behind.** Webcal uses the Rust 2018 layout: `foo.rs` alongside a `foo/` directory for submodules. If the target crate still has `foo/mod.rs`, `git mv` it to `foo.rs` so the structure matches.

## When this skill does NOT apply

- The provider already imports from `caldir_core::rpc::*` and uses `provider::Handler` → already migrated.
- The user wants to add a brand-new provider from scratch → use `docs/providers.md` directly, not this skill.
- The user wants to change the protocol itself (e.g. add a new RPC method) → that's a caldir-core change, not a provider migration.
